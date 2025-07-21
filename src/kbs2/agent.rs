use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use age::secrecy::{ExposeSecret as _, SecretString};
use anyhow::{anyhow, Context, Result};
use nix::unistd::Uid;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::kbs2::backend::{Backend, RageLib};

/// The version of the agent protocol.
const PROTOCOL_VERSION: u32 = 1;

/// Represents the entire request message, including the protocol field.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Request {
    protocol: u32,
    body: RequestBody,
}

/// Represents the kinds of requests understood by the `kbs2` authentication agent.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "body")]
enum RequestBody {
    /// Unwrap a particular keyfile (second element) with a password (third element), identifying
    /// it in the agent with a particular public key (first element).
    UnwrapKey(String, String, String),

    /// Check whether a particular public key has an unwrapped keyfile in the agent.
    QueryUnwrappedKey(String),

    /// Get the actual unwrapped key, by public key.
    GetUnwrappedKey(String),

    /// Flush all keys from the agent.
    FlushKeys,

    /// Ask the agent to exit.
    Quit,
}

/// Represents the kinds of responses sent by the `kbs2` authentication agent.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "body")]
enum Response {
    /// A successful request, with some request-specific response data.
    Success(String),

    /// A failed request, of `FailureKind`.
    Failure(FailureKind),
}

/// Represents the kinds of failures encoded by a `kbs2` `Response`.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "body")]
enum FailureKind {
    /// The request failed because the client couldn't be authenticated.
    Auth,

    /// The request failed because one or more I/O operations failed.
    Io(String),

    /// The request failed because it was malformed.
    Malformed(String),

    /// The request failed because key unwrapping failed.
    Unwrap(String),

    /// The request failed because the agent and client don't speak the same protocol version.
    VersionMismatch(u32),

    /// The request failed because the requested query failed.
    Query,
}

/// A convenience trait for marshaling and unmarshaling `RequestBody`s and `Response`s
/// through Rust's `Read` and `Write` traits.
trait Message {
    fn read<R: Read>(reader: R) -> Result<Self>
    where
        Self: DeserializeOwned,
    {
        // NOTE(ww): This would be cleaner with a BufReader, but unsound: a BufReader
        // can buffer more than one line at once, causing us to silently drop client requests.
        // I don't think that would actually happen in this case (since each client sends exactly
        // one line before expecting a response), but it's one less thing to think about.
        // NOTE(ww): Safe unwrap: we only perform after checking `is_ok`, and we capture
        // the error by using `Result<Vec<_>, _>` with `collect`.
        #[allow(clippy::unwrap_used, clippy::unbuffered_bytes)]
        let data: Result<Vec<_>, _> = reader
            .bytes()
            .take_while(|b| b.is_ok() && *b.as_ref().unwrap() != b'\n')
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
        writer.write_all(b"\n")?;
        writer.flush()?;

        Ok(())
    }
}

impl Message for Request {}
impl Message for Response {}

/// Represents the state in a running `kbs2` authentication agent.
pub struct Agent {
    /// The local path to the Unix domain socket.
    agent_path: PathBuf,
    /// A map of public key => (keyfile path, unwrapped key material).
    unwrapped_keys: HashMap<String, (String, SecretString)>,
    /// Whether or not the agent intends to quit momentarily.
    quitting: bool,
}

impl Agent {
    /// Returns a unique, user-specific socket path that the authentication agent listens on.
    fn path() -> PathBuf {
        let mut agent_path = PathBuf::from("/tmp");
        agent_path.push(format!("kbs2-agent-{}", whoami::username()));

        agent_path
    }

    /// Spawns a new agent as a daemon process, returning once the daemon
    /// is ready to begin serving clients.
    pub fn spawn() -> Result<()> {
        let agent_path = Self::path();

        // If an agent appears to be running already, do nothing.
        if agent_path.exists() {
            log::debug!("agent seems to be running; not trying to spawn another");
            return Ok(());
        }

        log::debug!("agent isn't already running, attempting spawn");

        // Sanity check: `kbs2` should never be run as root, and any difference between our
        // UID and EUID indicates some SUID-bit weirdness that we didn't expect and don't want.
        let (uid, euid) = (Uid::current(), Uid::effective());
        if uid.is_root() || uid != euid {
            return Err(anyhow!(
                "unusual UID or UID/EUID pair found, refusing to spawn"
            ));
        }

        // NOTE(ww): Given the above, it *should* be safe to spawn based on the path returned by
        // `current_exe`: we know we aren't being tricked with any hardlink + SUID shenanigans.
        let kbs2 = std::env::current_exe().with_context(|| "failed to locate the kbs2 binary")?;

        // NOTE(ww): We could spawn the agent by forking and daemonizing, but that would require
        // at least one direct use of unsafe{} (for the fork itself), and potentially others.
        // This is a little simpler and requires less unsafety.
        let _ = Command::new(kbs2)
            .arg("agent")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        for attempt in 0..10 {
            log::debug!("waiting for agent, loop {attempt}...");
            thread::sleep(Duration::from_millis(10));
            if agent_path.exists() {
                return Ok(());
            }
        }

        Err(anyhow!("agent spawn timeout exhausted"))
    }

    /// Initializes a new agent without accepting connections.
    pub fn new() -> Result<Self> {
        let agent_path = Self::path();
        if agent_path.exists() {
            return Err(anyhow!(
                "an agent is already running or didn't exit cleanly"
            ));
        }

        #[allow(clippy::redundant_field_names)]
        Ok(Self {
            agent_path: agent_path,
            unwrapped_keys: HashMap::new(),
            quitting: false,
        })
    }

    // TODO(ww): These can be replaced with the UnixStream.peer_cred API once it stabilizes:
    // https://doc.rust-lang.org/std/os/unix/net/struct.UnixStream.html#method.peer_cred
    #[cfg(any(target_os = "linux", target_os = "android",))]
    fn auth_client(&self, stream: &UnixStream) -> bool {
        use nix::sys::socket::getsockopt;
        use nix::sys::socket::sockopt::PeerCredentials;

        if let Ok(cred) = getsockopt(stream, PeerCredentials) {
            cred.uid() == Uid::effective().as_raw()
        } else {
            log::error!("getsockopt failed; treating as auth failure");
            false
        }
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly",
    ))]
    fn auth_client(&self, stream: &UnixStream) -> bool {
        use nix::unistd;

        if let Ok((peer_uid, _)) = unistd::getpeereid(stream) {
            peer_uid == Uid::effective()
        } else {
            log::error!("getpeereid failed; treating as auth failure");
            false
        }
    }

    /// Handles an inner request payload, i.e. one of potentially several
    /// requests made during a client's connection.
    fn handle_request_body(&mut self, body: RequestBody) -> Response {
        match body {
            RequestBody::UnwrapKey(pubkey, keyfile, password) => {
                let password = SecretString::from(password);
                // If the running agent is already tracking an unwrapped key for this
                // pubkey, return early with a success.
                #[allow(clippy::map_entry)]
                if self.unwrapped_keys.contains_key(&pubkey) {
                    log::debug!("client requested unwrap for already unwrapped keyfile: {keyfile}");
                    Response::Success("OK; agent already has unwrapped key".into())
                } else {
                    match RageLib::unwrap_keyfile(&keyfile, password) {
                        Ok(unwrapped_key) => {
                            self.unwrapped_keys.insert(pubkey, (keyfile, unwrapped_key));
                            Response::Success("OK; unwrapped key ready".into())
                        }
                        Err(e) => {
                            log::error!("keyfile unwrap failed: {e:?}");
                            Response::Failure(FailureKind::Unwrap(e.to_string()))
                        }
                    }
                }
            }
            RequestBody::QueryUnwrappedKey(pubkey) => {
                if self.unwrapped_keys.contains_key(&pubkey) {
                    Response::Success("OK".into())
                } else {
                    Response::Failure(FailureKind::Query)
                }
            }
            RequestBody::GetUnwrappedKey(pubkey) => {
                if let Some((_, unwrapped_key)) = self.unwrapped_keys.get(&pubkey) {
                    log::debug!("successful key request for pubkey: {pubkey}");
                    Response::Success(unwrapped_key.expose_secret().into())
                } else {
                    log::error!("unknown pubkey requested: {}", &pubkey);
                    Response::Failure(FailureKind::Query)
                }
            }
            RequestBody::FlushKeys => {
                self.unwrapped_keys.clear();
                log::debug!("successfully flushed all unwrapped keys");
                Response::Success("OK".into())
            }
            RequestBody::Quit => {
                self.quitting = true;
                log::debug!("agent exit requested");
                Response::Success("OK".into())
            }
        }
    }

    /// Handles a single client connection.
    /// Individual clients may issue multiple requests in a single session.
    fn handle_client(&mut self, stream: UnixStream) {
        let reader = BufReader::new(&stream);
        let mut writer = BufWriter::new(&stream);

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
                    log::error!("i/o error: {e:?}");
                    // This can fail, but we don't care.
                    let _ = Response::Failure(FailureKind::Io(e.to_string())).write(&mut writer);
                    return;
                }
            };

            let req: Request = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    log::error!("malformed req: {e:?}");
                    // This can fail, but we don't care.
                    let _ =
                        Response::Failure(FailureKind::Malformed(e.to_string())).write(&mut writer);
                    return;
                }
            };

            if req.protocol != PROTOCOL_VERSION {
                let _ = Response::Failure(FailureKind::VersionMismatch(PROTOCOL_VERSION))
                    .write(&mut writer);
                return;
            }

            let resp = self.handle_request_body(req.body);

            // This can fail, but we don't care.
            let _ = resp.write(&mut writer);
        }
    }

    /// Run the `kbs2` authentication agent.
    ///
    /// The function does not return *unless* either an error occurs on agent startup *or*
    /// a client asks the agent to quit.
    pub fn run(&mut self) -> Result<()> {
        log::debug!("agent run requested");

        let listener = UnixListener::bind(&self.agent_path)?;

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
                    self.handle_client(stream);
                    if self.quitting {
                        break;
                    }
                }
                Err(e) => {
                    log::error!("connect error: {e:?}");
                    continue;
                }
            }
        }

        Ok(())
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        log::debug!("agent teardown");

        // NOTE(ww): We don't expect this to fail, but it's okay if it does: the agent gets dropped
        // at the very end of its lifecycle, meaning that an expect here is acceptable.
        #[allow(clippy::expect_used)]
        fs::remove_file(Agent::path()).expect("attempted to remove missing agent socket");
    }
}

/// Represents a client to the `kbs2` authentication agent.
///
/// Clients may send multiple requests and receive multiple responses while active.
pub struct Client {
    stream: UnixStream,
}

impl Client {
    /// Create and return a new client, failing if connection to the agent fails.
    pub fn new() -> Result<Self> {
        log::debug!("creating a new agent client");

        let stream = UnixStream::connect(Agent::path())
            .with_context(|| "failed to connect to agent; is it running?")?;
        Ok(Self { stream })
    }

    /// Issue the given request to the agent, returning the agent's `Response`.
    fn request(&self, body: RequestBody) -> Result<Response> {
        #[allow(clippy::redundant_field_names)]
        let req = Request {
            protocol: PROTOCOL_VERSION,
            body: body,
        };
        req.write(&self.stream)?;
        let resp = Response::read(&self.stream)?;
        Ok(resp)
    }

    /// Instruct the agent to unwrap the given keyfile, using the given password.
    /// The keyfile path and its unwrapped contents are associated with the given pubkey.
    pub fn add_key(&self, pubkey: &str, keyfile: &str, password: SecretString) -> Result<()> {
        log::debug!("add_key: requesting that agent unwrap {keyfile}");

        let body = RequestBody::UnwrapKey(
            pubkey.into(),
            keyfile.into(),
            password.expose_secret().into(),
        );
        let resp = self.request(body)?;

        match resp {
            Response::Success(msg) => {
                log::debug!("agent reports success: {msg}");
                Ok(())
            }
            Response::Failure(kind) => Err(anyhow!("adding key to agent failed: {:?}", kind)),
        }
    }

    /// Ask the agent whether it has an unwrapped key for the given pubkey.
    pub fn query_key(&self, pubkey: &str) -> Result<bool> {
        log::debug!("query_key: asking whether agent has key for {pubkey}");

        let body = RequestBody::QueryUnwrappedKey(pubkey.into());
        let resp = self.request(body)?;

        match resp {
            Response::Success(_) => Ok(true),
            Response::Failure(FailureKind::Query) => Ok(false),
            Response::Failure(kind) => Err(anyhow!("querying key from agent failed: {:?}", kind)),
        }
    }

    /// Ask the agent for the unwrapped key material for the given pubkey.
    pub fn get_key(&self, pubkey: &str) -> Result<String> {
        log::debug!("get_key: requesting unwrapped key for {pubkey}");

        let body = RequestBody::GetUnwrappedKey(pubkey.into());
        let resp = self.request(body)?;

        match resp {
            Response::Success(unwrapped_key) => Ok(unwrapped_key),
            Response::Failure(kind) => Err(anyhow!(
                "retrieving unwrapped key from agent failed: {:?}",
                kind
            )),
        }
    }

    /// Ask the agent to flush all of its unwrapped keys.
    pub fn flush_keys(&self) -> Result<()> {
        log::debug!("flush_keys: asking agent to forget all keys");
        self.request(RequestBody::FlushKeys)?;
        Ok(())
    }

    /// Ask the agent to quit gracefully.
    pub fn quit_agent(self) -> Result<()> {
        log::debug!("quit_agent: asking agent to exit gracefully");
        self.request(RequestBody::Quit)?;
        Ok(())
    }
}
