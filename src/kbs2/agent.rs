use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::kbs2::session::Session;

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

struct Agent {
    unwrapped_keys: HashMap<String, String>,
}

impl Agent {
    fn new() -> Self {
        Self {
            unwrapped_keys: HashMap::new(),
        }
    }

    fn unwrap_keyfile(&self, keyfile: &str, password: &str) -> Result<String> {
        unimplemented!();
    }

    fn handle_client(&mut self, stream: UnixStream) {
        let req: Request = match serde_json::from_reader(&stream) {
            Ok(req) => req,
            Err(e) => {
                let resp = Response::Failure(format!("malformed client request: {:?}", e));
                log::error!("{:?}", resp);
                // This can fail, but we don't care.
                let _ = serde_json::to_writer(&stream, &resp);
                return;
            }
        };

        let resp = match req {
            Request::UnwrapKey(keyfile, password) => {
                // If the running agent is already tracking an unwrapped key for this
                // keyfile, return early with a success.
                if self.unwrapped_keys.contains_key(&keyfile) {
                    log::debug!(
                        "client requested unwrap for already unwrapped keyfile: {}",
                        keyfile
                    );
                    Response::Success("OK; agent already has unwrapped key".into())
                } else {
                    match self.unwrap_keyfile(&keyfile, &password) {
                        Ok(unwrapped_key) => {
                            self.unwrapped_keys.insert(keyfile, unwrapped_key);
                            Response::Success("OK; unwrapped key ready".into())
                        },
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
                    Response::Success(unwrapped_key.into())
                } else {
                    log::error!("unknown keyfile requested: {}", &keyfile);
                    Response::Failure("no unwrapped key for that keyfile".into())
                }
            }
            Request::FlushKeys => unimplemented!(),
            Request::Quit => unimplemented!(),
        };

        let _ = serde_json::to_writer(&stream, &resp);
    }
}

fn path() -> PathBuf {
    let mut agent_path = PathBuf::from("/tmp");
    agent_path.push(format!("kbs2-agent-{}", whoami::username()));

    agent_path
}

pub fn run(session: &Session) -> Result<()> {
    log::debug!("agent run requested");

    let agent_path = path();
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
