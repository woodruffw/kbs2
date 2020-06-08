use anyhow::{anyhow, Result};
use atty::Stream;
use clap::ArgMatches;
use clipboard::{ClipboardContext, ClipboardProvider};
use nix::errno::Errno;
use nix::sys::mman;
use nix::unistd::{fork, ForkResult};
use tempfile;

use std::env;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process;

use crate::kbs2::config;
use crate::kbs2::generator::Generator;
use crate::kbs2::input;
use crate::kbs2::record::{self, FieldKind::*, RecordBody};
use crate::kbs2::session;
use crate::kbs2::util;

pub fn init(matches: &ArgMatches, config_dir: &Path) -> Result<()> {
    log::debug!("initializing a new config");

    if config_dir.join(config::CONFIG_BASENAME).exists() && !matches.is_present("force") {
        return Err(anyhow!("refusing to overwrite your current config without --force"));
    }

    config::initialize(&config_dir, !matches.is_present("insecure-not-wrapped"))
}

pub fn unlock(_matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("unlock requested");

    if !config.wrapped {
        return Err(anyhow!("unlock requested but wrapped=false in config"));
    }

    // NOTE(ww): All of the unwrapping happens in unwrap_keyfile.
    // The unwrapped data is persistent in shared memory once we return successfully.
    config.unwrap_keyfile()?;

    Ok(())
}

pub fn lock(_matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("lock requested");

    if !config.wrapped {
        util::warn("config says that key isn't wrapped, trying anyways...");
    }

    match mman::shm_unlink(config::UNWRAPPED_KEY_SHM_NAME) {
        Ok(()) => Ok(()),
        Err(nix::Error::Sys(Errno::ENOENT)) => Err(anyhow!("no unwrapped key to remove")),
        Err(e) => Err(e.into()),
    }
}

pub fn new(matches: &ArgMatches, session: &session::Session) -> Result<()> {
    log::debug!("creating a new record");

    if let Some(pre_hook) = &session.config.commands.new.pre_hook {
        log::debug!("pre-hook: {}", pre_hook);
        session.config.call_hook(pre_hook, &[])?;
    }

    let label = matches.value_of("label").unwrap();
    if session.has_record(label) && !matches.is_present("force") {
        return Err(anyhow!("refusing to overwrite a record without --force"));
    }

    let terse = atty::isnt(Stream::Stdin) || matches.is_present("terse");

    let generator = if matches.is_present("generate") {
        let generator_name = matches.value_of("generator").unwrap();

        Some(session.config.get_generator(generator_name).ok_or(anyhow!(
            "couldn't find a generator named {}",
            generator_name
        ))?)
    } else {
        None
    };

    // TODO: new_* below is a little silly. This should be de-duped.
    match matches.value_of("kind").unwrap() {
        "login" => new_login(label, terse, &session, &generator)?,
        "environment" => new_environment(label, terse, &session, &generator)?,
        "unstructured" => new_unstructured(label, terse, &session, &generator)?,
        _ => unreachable!(),
    }

    if let Some(post_hook) = &session.config.commands.new.post_hook {
        log::debug!("post-hook: {}", post_hook);
        session.config.call_hook(post_hook, &[&label])?;
    }

    Ok(())
}

fn new_login(
    label: &str,
    terse: bool,
    session: &session::Session,
    generator: &Option<Box<&dyn Generator>>,
) -> Result<()> {
    let fields = input::fields(
        &[Insensitive("Username"), Sensitive("Password")],
        terse,
        &generator,
    )?;
    let record = record::Record::login(label, &fields[0], &fields[1]);

    session.add_record(&record)
}

fn new_environment(
    label: &str,
    terse: bool,
    session: &session::Session,
    generator: &Option<Box<&dyn Generator>>,
) -> Result<()> {
    let fields = input::fields(
        &[Insensitive("Variable"), Sensitive("Value")],
        terse,
        &generator,
    )?;
    let record = record::Record::environment(label, &fields[0], &fields[1]);

    session.add_record(&record)
}

fn new_unstructured(
    label: &str,
    terse: bool,
    session: &session::Session,
    generator: &Option<Box<&dyn Generator>>,
) -> Result<()> {
    let fields = input::fields(&[Insensitive("Contents")], terse, &generator)?;
    let record = record::Record::unstructured(label, &fields[0]);

    session.add_record(&record)
}

pub fn list(matches: &ArgMatches, session: &session::Session) -> Result<()> {
    log::debug!("listing records");

    let (details, filter_kind) = (matches.is_present("details"), matches.is_present("kind"));

    for label in session.record_labels()? {
        let mut display = String::new();

        if details || filter_kind {
            let record = session.get_record(&label)?;

            if filter_kind {
                let kind = matches.value_of("kind").unwrap();
                if record.body.to_string() != kind {
                    continue;
                }
            }

            display.push_str(&label);

            if details {
                display.push_str(&format!(
                    "\n\tKind: {}\n\tTimestamp: {}",
                    record.body, record.timestamp
                ));
            }
        } else {
            display.push_str(&label);
        }

        println!("{}", display);
    }

    Ok(())
}

pub fn rm(matches: &ArgMatches, session: &session::Session) -> Result<()> {
    log::debug!("removing a record");

    let label = matches.value_of("label").unwrap();
    session.delete_record(label)?;

    if let Some(post_hook) = &session.config.commands.rm.post_hook {
        log::debug!("post-hook: {}", post_hook);
        session.config.call_hook(post_hook, &[&label])?;
    }

    Ok(())
}

pub fn dump(matches: &ArgMatches, session: &session::Session) -> Result<()> {
    log::debug!("dumping a record");

    let label = matches.value_of("label").unwrap();
    let record = session.get_record(&label)?;

    if matches.is_present("json") {
        println!("{}", serde_json::to_string(&record)?);
    } else {
        println!("Label: {}\n\tKind: {}", label, record.body);

        match record.body {
            RecordBody::Login(l) => {
                println!("\tUsername: {}\n\tPassword: {}", l.username, l.password)
            }
            RecordBody::Environment(e) => {
                println!("\tVariable: {}\n\tValue: {}", e.variable, e.value)
            }
            RecordBody::Unstructured(u) => println!("\tContents: {}", u.contents),
        }
    }

    Ok(())
}

pub fn pass(matches: &ArgMatches, session: &session::Session) -> Result<()> {
    log::debug!("getting a login's password");

    if let Some(pre_hook) = &session.config.commands.pass.pre_hook {
        log::debug!("pre-hook: {}", pre_hook);
        session.config.call_hook(pre_hook, &[])?;
    }

    let label = matches.value_of("label").unwrap();
    let record = session.get_record(&label)?;

    let login = match record.body {
        RecordBody::Login(l) => l,
        _ => return Err(anyhow!("not a login record: {}", label)),
    };

    let password = login.password;
    if matches.is_present("clipboard") {
        let clipboard_duration = session.config.commands.pass.clipboard_duration;
        let clear_after = session.config.commands.pass.clear_after;

        // NOTE(ww): We fork here for two reasons: one X11 specific, and one general.
        //
        // 1. X11's clipboard's are tied to processes, meaning that they disappear when the
        //    creating process terminates. There are ways around that, but the clipboard
        //    crate doesn't implement them in the interest of simplicity. Therefore, we
        //    fork to ensure that a process outlives our "main" kbs2 process for pasting purposes.
        // 2. Forking gives us a way to clear the password from the clipboard after
        //    a particular duration, without resorting to an external daemon or other service.
        match fork() {
            Ok(ForkResult::Child) => {
                // TODO(ww): Support x11_clipboard config option.
                if session.config.commands.pass.x11_clipboard == config::X11Clipboard::Primary {
                    util::warn("primary clipboard requested but not yet supported");
                }

                let mut ctx: ClipboardContext =
                    ClipboardProvider::new().map_err(|_| anyhow!("unable to grab the clipboard"))?;

                ctx.set_contents(password.to_owned())
                    .map_err(|_| anyhow!("unable to store to the clipboard"))?;

                std::thread::sleep(std::time::Duration::from_secs(clipboard_duration));

                if clear_after {
                    ctx.set_contents("".to_owned())
                        .map_err(|_| anyhow!("unable to clear the clipboard"))?;

                    if let Some(clear_hook) = &session.config.commands.pass.clear_hook {
                        log::debug!("clear-hook: {}", clear_hook);
                        session.config.call_hook(clear_hook, &[])?;
                    }
                }
            }
            Err(_) => return Err(anyhow!("clipboard fork failed")),
            _ => {}
        }
    } else {
        if atty::isnt(Stream::Stdout) {
            print!("{}", password);
        } else {
            println!("{}", password);
        }
    }

    if let Some(post_hook) = &session.config.commands.pass.post_hook {
        log::debug!("post-hook: {}", post_hook);
        session.config.call_hook(post_hook, &[])?;
    }

    Ok(())
}

pub fn env(matches: &ArgMatches, session: &session::Session) -> Result<()> {
    log::debug!("getting a environment variable");

    let label = matches.value_of("label").unwrap();
    let record = session.get_record(&label)?;

    let environment = match record.body {
        RecordBody::Environment(e) => e,
        _ => return Err(anyhow!("not an environment record: {}", label)),
    };

    if matches.is_present("value-only") {
        println!("{}", environment.value);
    } else {
        if matches.is_present("no-export") {
            println!("{}={}", environment.variable, environment.value);
        } else {
            println!("export {}={}", environment.variable, environment.value);
        }
    }

    Ok(())
}

pub fn edit(matches: &ArgMatches, session: &session::Session) -> Result<()> {
    log::debug!("editing a record");

    let editor = match session
        .config
        .commands
        .edit
        .editor
        .as_ref()
        .cloned()
        .or_else(|| env::var("EDITOR").ok())
    {
        Some(editor) => editor,
        None => return Err(anyhow!("no editor configured to edit with")),
    };

    let (editor, editor_args) = util::parse_and_split_args(&editor)?;

    log::debug!("editor: {}, args: {:?}", editor, editor_args);

    let label = matches.value_of("label").unwrap();
    let record = session.get_record(&label)?;

    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(&serde_json::to_vec_pretty(&record)?)?;

    if !process::Command::new(&editor)
        .args(&editor_args)
        .arg(file.path())
        .output()
        .map_or(false, |o| o.status.success())
    {
        return Err(anyhow!("failed to run the editor"));
    }

    // Rewind, pull the changed contents, deserialize back into a record.
    file.seek(SeekFrom::Start(0))?;
    let mut record_contents = vec![];
    file.read_to_end(&mut record_contents)?;

    let mut record = serde_json::from_slice::<record::Record>(&record_contents)?;

    // Users can't modify these fields, at least not with `kbs2 edit`.
    record.label = label.into();
    record.timestamp = util::current_timestamp();

    session.add_record(&record)?;

    if let Some(post_hook) = &session.config.commands.edit.post_hook {
        log::debug!("post-hook: {}", post_hook);
        session.config.call_hook(post_hook, &[])?;
    }

    Ok(())
}

pub fn generate(matches: &ArgMatches, session: &session::Session) -> Result<()> {
    let generator = {
        let generator_name = matches.value_of("generator").unwrap();
        match session.config.get_generator(generator_name) {
            Some(generator) => generator,
            None => {
                return Err(anyhow!("couldn't find a generator named {}", generator_name))
            }
        }
    };

    Ok(println!("{}", generator.secret()?))
}
