use age::Decryptor;
use anyhow::{anyhow, Result};
use secrecy::{ExposeSecret, Secret, SecretString};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
enum Request {
    UnwrapKey(String, String),
    GetUnwrappedKey(String),
    FlushKeys,
    Quit,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
enum Response {
    Success(String),
    Failure(String),
}

trait Message {
    fn read<R: Read>(reader: R) -> Result<Self>
    where
        Self: DeserializeOwned,
    {
        let mut data = String::new();
        let mut reader = BufReader::new(reader);
        reader.read_line(&mut data)?;
        let res = serde_json::from_str(&data)?;

        Ok(res)
    }

    fn write<W: Write>(&self, mut writer: W) -> Result<()>
    where
        Self: Serialize,
    {
        serde_json::to_writer(&mut writer, &self)?;
        writer.write_all(&[b'\n'])?;

        Ok(())
    }
}

impl Message for Request {}
impl Message for Response {}

struct Agent {
    unwrapped_keys: HashMap<String, SecretString>,
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
            })
            .or_else(|e| Err(e))?;
        log::debug!("finished key unwrap!");

        Ok(Secret::new(unwrapped_key))
    }

    fn handle_client(&mut self, stream: UnixStream) {
        let mut writer = BufWriter::new(&stream);

        let req = match Request::read(&stream) {
            Ok(req) => req,
            Err(e) => {
                let resp = Response::Failure(format!("malformed client request: {:?}", e));
                log::error!("{:?}", resp);
                // This can fail, but we don't care.
                let _ = resp.write(&mut writer);
                return;
            }
        };

        let resp = match req {
            Request::UnwrapKey(keyfile, password) => {
                let password = Secret::new(password);
                // If the running agent is already tracking an unwrapped key for this
                // keyfile, return early with a success.
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
                            Response::Failure(format!("keyfile unwrap failed: {:?}", e))
                        }
                    }
                }
            }
            Request::GetUnwrappedKey(keyfile) => {
                if let Some(unwrapped_key) = self.unwrapped_keys.get(&keyfile) {
                    log::debug!("successful key request for keyfile: {}", keyfile);
                    Response::Success(unwrapped_key.expose_secret().into())
                } else {
                    log::error!("unknown keyfile requested: {}", &keyfile);
                    Response::Failure("no unwrapped key for that keyfile".into())
                }
            }
            Request::FlushKeys => {
                self.unwrapped_keys.clear();
                log::debug!("successfully flushed all unwrapped keys");
                Response::Success("OK".into())
            }
            Request::Quit => unimplemented!(),
        };

        let _ = resp.write(&mut writer);
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

    fn request(&mut self, req: &Request) -> Result<Response> {
        req.write(&self.stream)?;
        let resp = Response::read(&self.stream)?;
        Ok(resp)
    }

    pub fn add_key(&mut self, keyfile: &str, password: SecretString) -> Result<()> {
        let req = Request::UnwrapKey(keyfile.into(), password.expose_secret().into());
        let resp = self.request(&req)?;

        match resp {
            Response::Success(msg) => {
                log::debug!("agent reports success: {}", msg);
                Ok(())
            }
            Response::Failure(msg) => {
                log::debug!("agent reports failure: {}", msg);
                Err(anyhow!(msg))
            }
        }
    }

    pub fn get_key(&mut self, keyfile: &str) -> Result<String> {
        let req = Request::GetUnwrappedKey(keyfile.into());
        let resp = self.request(&req)?;

        match resp {
            Response::Success(unwrapped_key) => Ok(unwrapped_key),
            Response::Failure(msg) => Err(anyhow!(msg)),
        }
    }
}

pub fn run() -> Result<()> {
    log::debug!("agent run requested");

    let agent_path = Agent::path();
    if agent_path.exists() {
        return Err(anyhow!("an agent is already running for this user"));
    }

    let agent = Arc::new(Mutex::new(Agent::new()));
    let listener = UnixListener::bind(&agent_path)?;

    for stream in listener.incoming() {
        let agent = Arc::clone(&agent);
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    let mut agent = agent.lock().unwrap();
                    agent.handle_client(stream)
                });
            }
            Err(e) => {
                log::error!("connect error: {:?}", e);
                continue;
            }
        }
    }

    Ok(())
}
