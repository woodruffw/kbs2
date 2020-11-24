use age::Decryptor;
use anyhow::{anyhow, Result};
use nix::unistd::geteuid;
use secrecy::{ExposeSecret, Secret, SecretString};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "body")]
enum Request {
    UnwrapKey(String, String),
    QueryUnwrappedKey(String),
    GetUnwrappedKey(String),
    FlushKeys,
    Quit,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "body")]
enum FailureKind {
    Auth,
    Io(String),
    Malformed(String),
    Unwrap(String),
    Query,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "body")]
enum Response {
    Success(String),
    Failure(FailureKind),
}

trait Message {
    fn read<R: Read>(reader: R) -> Result<Self>
    where
        Self: DeserializeOwned,
    {
        // NOTE(ww): This would be cleaner with a BufReader, but unsafe: a BufReader
        // can buffer more than one line at once, causing us to silently drop client requests.
        // I don't think that would actually happen in this case (since each client sends exactly
        // one line before expecting a response), but it's one less thing to think about.
        let data: Result<Vec<u8>, _> = reader
            .bytes()
            .take_while(|b| *b.as_ref().unwrap() != b'\n')
            .collect();
        let data = data?;
        let res = serde_json::from_slice(&data)?;

        Ok(res)
    }

    fn write<W: Write>(&self, mut writer: W) -> Result<()>
    where
        Self: Serialize,
    {
        serde_json::to_writer(&mut writer, &self)?;
        writer.write_all(&[b'\n'])?;
        writer.flush()?;

        Ok(())
    }
}

impl Message for Request {}
impl Message for Response {}

struct Agent {
    unwrapped_keys: HashMap<String, SecretString>,
    quitting: bool,
}

impl Agent {
    fn path() -> PathBuf {
        let mut agent_path = PathBuf::from("/tmp");
        agent_path.push(format!("kbs2-agent-{}", whoami::username()));

        agent_path
    }

    fn new() -> Self {
        Self {
            unwrapped_keys: HashMap::new(),
            quitting: false,
        }
    }

    fn unwrap_keyfile(&self, keyfile: &str, password: SecretString) -> Result<SecretString> {
        // TODO(ww): Hardening: check keyfile's size before reading the whole thing in.
        // Read the wrapped key from disk.
        let wrapped_key = fs::read(&keyfile)?;

        // Create a new decryptor for the wrapped key.
        let decryptor = match Decryptor::new(wrapped_key.as_slice()) {
            Ok(Decryptor::Passphrase(d)) => d,
            Ok(_) => {
                return Err(anyhow!(
                    "key unwrap failed; not a password-wrapped keyfile?"
                ));
            }
            Err(e) => {
                return Err(anyhow!(
                    "unable to load private key (backend reports: {:?})",
                    e
                ));
            }
        };

        // ...and decrypt (i.e., unwrap) using the master password.
        log::debug!("beginning key unwrap...");
        let mut unwrapped_key = String::new();

        // NOTE(ww): A work factor of 18 is an educated guess here; rage generated some
        // encrypted messages that needed this factor.
        decryptor
            .decrypt(&password, Some(18))
            .map_err(|e| anyhow!("unable to decrypt (backend reports: {:?})", e))
            .and_then(|mut r| {
                r.read_to_string(&mut unwrapped_key)
                    .map_err(|_| anyhow!("i/o error while decrypting"))
            })?;
        log::debug!("finished key unwrap!");

        Ok(Secret::new(unwrapped_key))
    }

    // TODO(ww): These can be replaced with the UnixStream.peer_cred API once it stabilizes:
    // https://doc.rust-lang.org/std/os/unix/net/struct.UnixStream.html#method.peer_cred
    #[cfg(target_os = "linux")]
    fn auth_client(&self, stream: &UnixStream) -> bool {
        use nix::sys::socket::getsockopt;
        use nix::sys::socket::sockopt::PeerCredentials;

        let uid = geteuid().as_raw();
        if let Ok(cred) = getsockopt(stream.as_raw_fd(), PeerCredentials) {
            cred.uid() == uid
        } else {
            log::error!("getsockopt failed; treating as auth failure");
            false
        }
    }

    #[cfg(target_os = "macos")]
    fn auth_client(&self, stream: &UnixStream) -> bool {
        let uid = geteuid().as_raw();
        let mut peer_uid = 1;
        let mut peer_gid = 1;

        unsafe {
            let ret = libc::getpeereid(stream.as_raw_fd(), &mut peer_uid, &mut peer_gid);
            if ret == 0 {
                uid == peer_uid
            } else {
                log::debug!("getpeereid failed; treating as auth failure");
                false
            }
        }
    }

    fn handle_client(&mut self, stream: UnixStream) {
        let reader = BufReader::new(&stream);
        let mut writer = BufWriter::new(&stream);

        // TODO: verify client here.
        if !self.auth_client(&stream) {
            log::warn!("client failed auth check");
            // This can fail, but we don't care.
            let _ = Response::Failure(FailureKind::Auth).write(&mut writer);
            return;
        }

        for line in reader.lines() {
            let line = match line {
                Ok(line) => line,
                Err(e) => {
                    log::error!("i/o error: {:?}", e);
                    // This can fail, but we don't care.
                    let _ = Response::Failure(FailureKind::Io(e.to_string())).write(&mut writer);
                    return;
                }
            };

            let req = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    log::error!("malformed req: {:?}", e);
                    // This can fail, but we don't care.
                    let _ =
                        Response::Failure(FailureKind::Malformed(e.to_string())).write(&mut writer);
                    return;
                }
            };

            let resp = match req {
                Request::UnwrapKey(keyfile, password) => {
                    let password = Secret::new(password);
                    // If the running agent is already tracking an unwrapped key for this
                    // keyfile, return early with a success.
                    #[allow(clippy::map_entry)]
                    if self.unwrapped_keys.contains_key(&keyfile) {
                        log::debug!(
                            "client requested unwrap for already unwrapped keyfile: {}",
                            keyfile
                        );
                        Response::Success("OK; agent already has unwrapped key".into())
                    } else {
                        match self.unwrap_keyfile(&keyfile, password) {
                            Ok(unwrapped_key) => {
                                self.unwrapped_keys.insert(keyfile, unwrapped_key);
                                Response::Success("OK; unwrapped key ready".into())
                            }
                            Err(e) => {
                                log::error!("keyfile unwrap failed: {:?}", e);
                                Response::Failure(FailureKind::Unwrap(e.to_string()))
                            }
                        }
                    }
                }
                Request::QueryUnwrappedKey(keyfile) => {
                    if self.unwrapped_keys.contains_key(&keyfile) {
                        Response::Success("OK".into())
                    } else {
                        Response::Failure(FailureKind::Query)
                    }
                }
                Request::GetUnwrappedKey(keyfile) => {
                    if let Some(unwrapped_key) = self.unwrapped_keys.get(&keyfile) {
                        log::debug!("successful key request for keyfile: {}", keyfile);
                        Response::Success(unwrapped_key.expose_secret().into())
                    } else {
                        log::error!("unknown keyfile requested: {}", &keyfile);
                        Response::Failure(FailureKind::Query)
                    }
                }
                Request::FlushKeys => {
                    self.unwrapped_keys.clear();
                    log::debug!("successfully flushed all unwrapped keys");
                    Response::Success("OK".into())
                }
                Request::Quit => {
                    self.quitting = true;
                    log::debug!("agent exit requested");
                    Response::Success("OK".into())
                }
            };

            let _ = resp.write(&mut writer);
        }
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        log::debug!("agent teardown");
        fs::remove_file(Agent::path()).expect("attempted to remove missing agent socket");
    }
}

pub struct Client {
    stream: UnixStream,
}

impl Client {
    pub fn new() -> Result<Self> {
        let stream = UnixStream::connect(Agent::path())?;
        Ok(Self { stream })
    }

    fn request(&self, req: &Request) -> Result<Response> {
        req.write(&self.stream)?;
        let resp = Response::read(&self.stream)?;
        Ok(resp)
    }

    pub fn add_key(&self, keyfile: &str, password: SecretString) -> Result<()> {
        log::debug!("add_key: requesting that agent unwrap {}", keyfile);

        let req = Request::UnwrapKey(keyfile.into(), password.expose_secret().into());
        let resp = self.request(&req)?;

        match resp {
            Response::Success(msg) => {
                log::debug!("agent reports success: {}", msg);
                Ok(())
            }
            Response::Failure(kind) => Err(anyhow!("adding key to agent failed: {:?}", kind)),
        }
    }

    pub fn query_key(&self, keyfile: &str) -> Result<bool> {
        log::debug!("query_key: asking whether client has key for {}", keyfile);

        let req = Request::QueryUnwrappedKey(keyfile.into());
        let resp = self.request(&req)?;

        match resp {
            Response::Success(_) => Ok(true),
            Response::Failure(FailureKind::Query) => Ok(false),
            Response::Failure(kind) => Err(anyhow!("querying key from agent failed: {:?}", kind)),
        }
    }

    pub fn get_key(&self, keyfile: &str) -> Result<String> {
        log::debug!("get_key: requesting unwrapped key for {}", keyfile);

        let req = Request::GetUnwrappedKey(keyfile.into());
        let resp = self.request(&req)?;

        match resp {
            Response::Success(unwrapped_key) => Ok(unwrapped_key),
            Response::Failure(kind) => Err(anyhow!(
                "retrieving unwrapped key from agent failed: {:?}",
                kind
            )),
        }
    }

    pub fn flush_keys(&self) -> Result<()> {
        log::debug!("flush_keys: asking agent to forget all keys");
        self.request(&Request::FlushKeys)?;
        Ok(())
    }

    pub fn quit_agent(self) -> Result<()> {
        log::debug!("quit_agent: asking agent to exit gracefully");
        self.request(&Request::Quit)?;
        Ok(())
    }
}

pub fn run() -> Result<()> {
    log::debug!("agent run requested");

    let agent_path = Agent::path();
    if agent_path.exists() {
        return Err(anyhow!(
            "an agent is already running or didn't exit cleanly"
        ));
    }

    let mut agent = Agent::new();
    let listener = UnixListener::bind(&agent_path)?;

    // NOTE(ww): This could spawn a separate thread for each incoming connection, but I see
    // no reason to do so:
    //
    // 1. The incoming queue already provides a synchronization mechanism, and we don't
    //    expect a number of simultaneous clients that would come close to exceeding the
    //    default queue length. Even if that were to happen, rejecting pending clients
    //    is an acceptable error mode.
    // 2. Using separate threads here makes the rest of the code unnecessarily complicated:
    //    each `Agent` becomes an `Arc<Mutex<Agent>>` to protect the underlying `HashMap`,
    //    and makes actually quitting the agent with a `Quit` request more difficult than it
    //    needs to be.
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                agent.handle_client(stream);
                if agent.quitting {
                    break;
                }
            }
            Err(e) => {
                log::error!("connect error: {:?}", e);
                continue;
            }
        }
    }

    Ok(())
}
