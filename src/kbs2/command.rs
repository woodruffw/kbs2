use anyhow::{anyhow, Result};
use atty::Stream;
use clap::ArgMatches;
use clipboard::{ClipboardContext, ClipboardProvider};
use daemonize::Daemonize;
use nix::unistd::{fork, ForkResult};

use std::env;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process;

use crate::kbs2::agent;
use crate::kbs2::config;
use crate::kbs2::generator::Generator;
use crate::kbs2::input;
use crate::kbs2::record::{self, FieldKind::*, RecordBody};
use crate::kbs2::session;
use crate::kbs2::util;

/// Implements the `kbs2 init` command.
pub fn init(matches: &ArgMatches, config_dir: &Path) -> Result<()> {
    log::debug!("initializing a new config");

    if config_dir.join(config::CONFIG_BASENAME).exists() && !matches.is_present("force") {
        return Err(anyhow!(
            "refusing to overwrite your current config without --force"
        ));
    }

    config::initialize(&config_dir, !matches.is_present("insecure-not-wrapped"))
}

/// Implements the `kbs2 agent` command (and subcommands).
pub fn agent(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("agent subcommand dispatch");

    if matches.subcommand().is_none() {
        if !matches.is_present("foreground") {
            Daemonize::new().start()?;
        }
        agent::run()?;
        return Ok(());
    }

    // No subcommand: run the agent itself
    match matches.subcommand() {
        Some(("flush", matches)) => agent_flush(&matches),
        Some(("unwrap", matches)) => agent_unwrap(&matches, &config),
        _ => unreachable!(),
    }
}

/// Implements the `kbs2 agent flush` subcommand.
fn agent_flush(matches: &ArgMatches) -> Result<()> {
    log::debug!("asking the agent to flush all keys");

    let client = agent::Client::new()?;
    client.flush_keys()?;

    if matches.is_present("quit") {
        client.quit_agent()?;
    }

    Ok(())
}

/// Implements the `kbs2 agent unwrap` subcommand.
fn agent_unwrap(_matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("asking the agent to unwrap a key");

    // Bare keys are loaded directly from their `keyfile`.
    if !config.wrapped {
        return Err(anyhow!("config specifies a bare key; nothing to do"));
    }

    let client = agent::Client::new()?;
    if client.query_key(&config.keyfile)? {
        println!("kbs2 agent already has this key; ignoring.");
        return Ok(());
    }

    let password = util::get_password()?;
    client.add_key(&config.keyfile, password)?;

    Ok(())
}

/// Implements the `kbs2 new` command.
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

        Some(
            session
                .config
                .get_generator(generator_name)
                .ok_or_else(|| anyhow!("couldn't find a generator named {}", generator_name))?,
        )
    } else {
        None
    };

    // TODO: new_* below is a little silly. This should be de-duped.
    match matches.value_of("kind").unwrap() {
        "login" => new_login(label, terse, &session, generator)?,
        "environment" => new_environment(label, terse, &session, generator)?,
        "unstructured" => new_unstructured(label, terse, &session, generator)?,
        _ => unreachable!(),
    }

    if let Some(post_hook) = &session.config.commands.new.post_hook {
        log::debug!("post-hook: {}", post_hook);
        session.config.call_hook(post_hook, &[&label])?;
    }

    Ok(())
}

#[doc(hidden)]
fn new_login(
    label: &str,
    terse: bool,
    session: &session::Session,
    generator: Option<&dyn Generator>,
) -> Result<()> {
    let fields = input::fields(
        &[Insensitive("Username"), Sensitive("Password")],
        terse,
        &session.config,
        generator,
    )?;
    let record = record::Record::login(label, &fields[0], &fields[1]);

    session.add_record(&record)
}

#[doc(hidden)]
fn new_environment(
    label: &str,
    terse: bool,
    session: &session::Session,
    generator: Option<&dyn Generator>,
) -> Result<()> {
    let fields = input::fields(
        &[Insensitive("Variable"), Sensitive("Value")],
        terse,
        &session.config,
        generator,
    )?;
    let record = record::Record::environment(label, &fields[0], &fields[1]);

    session.add_record(&record)
}

#[doc(hidden)]
fn new_unstructured(
    label: &str,
    terse: bool,
    session: &session::Session,
    generator: Option<&dyn Generator>,
) -> Result<()> {
    let fields = input::fields(
        &[Insensitive("Contents")],
        terse,
        &session.config,
        generator,
    )?;
    let record = record::Record::unstructured(label, &fields[0]);

    session.add_record(&record)
}

/// Implements the `kbs2 list` command.
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

/// Implements the `kbs2 rm` command.
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

/// Implements the `kbs2 dump` command.
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

/// Implements the `kbs2 pass` command.
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
        // NOTE(ww): fork() is unsafe in multithreaded programs where the child calls
        // non async-signal-safe functions. kbs2 is single threaded, so this usage is fine.
        unsafe {
            match fork() {
                Ok(ForkResult::Child) => {
                    // NOTE(ww): More dumbness: cfg! gets expanded into a boolean literal,
                    // so it can't be used to conditionally compile code that only exists on
                    // one platform.
                    #[cfg(target_os = "linux")]
                    {
                        match session.config.commands.pass.x11_clipboard {
                            // NOTE(ww): Why, might you ask, is clip_primary its own function?
                            // It's because the clipboard crate has a bad abstraction:
                            // ClipboardContext is the top-level type, but it's aliased to
                            // X11Clipboard<Clipboard>. That means we can't produce it on a match.
                            // The other option would be to create a ClipboardProvider trait object,
                            // but it doesn't implement Sized. So we have to do things the dumb
                            // way here. Alternatively, I could just be missing something obvious.
                            config::X11Clipboard::Primary => clip_primary(password, &session)?,
                            config::X11Clipboard::Clipboard => clip(password, &session)?,
                        };
                    }

                    #[cfg(target_os = "macos")]
                    {
                        clip(password, &session)?;
                    }
                }
                Err(_) => return Err(anyhow!("clipboard fork failed")),
                _ => {}
            }
        }
    } else if atty::isnt(Stream::Stdout) {
        print!("{}", password);
    } else {
        println!("{}", password);
    }

    if let Some(post_hook) = &session.config.commands.pass.post_hook {
        log::debug!("post-hook: {}", post_hook);
        session.config.call_hook(post_hook, &[])?;
    }

    Ok(())
}

#[doc(hidden)]
fn clip(password: String, session: &session::Session) -> Result<()> {
    let clipboard_duration = session.config.commands.pass.clipboard_duration;
    let clear_after = session.config.commands.pass.clear_after;

    let mut ctx: ClipboardContext =
        ClipboardProvider::new().map_err(|_| anyhow!("unable to grab the clipboard"))?;
    ctx.set_contents(password)
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

    Ok(())
}

#[doc(hidden)]
#[cfg(target_os = "linux")]
fn clip_primary(password: String, session: &session::Session) -> Result<()> {
    use clipboard::x11_clipboard::{Primary, X11ClipboardContext};

    let clipboard_duration = session.config.commands.pass.clipboard_duration;
    let clear_after = session.config.commands.pass.clear_after;

    let mut ctx: X11ClipboardContext<Primary> =
        ClipboardProvider::new().map_err(|_| anyhow!("unable to grab the clipboard"))?;
    ctx.set_contents(password)
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

    Ok(())
}

/// Implements the `kbs2 env` command.
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
    } else if matches.is_present("no-export") {
        println!("{}={}", environment.variable, environment.value);
    } else {
        println!("export {}={}", environment.variable, environment.value);
    }

    Ok(())
}

/// Implements the `kbs2 edit` command.
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

/// Implements the `kbs2 generate` command.
pub fn generate(matches: &ArgMatches, session: &session::Session) -> Result<()> {
    let generator = {
        let generator_name = matches.value_of("generator").unwrap();
        match session.config.get_generator(generator_name) {
            Some(generator) => generator,
            None => {
                return Err(anyhow!(
                    "couldn't find a generator named {}",
                    generator_name
                ))
            }
        }
    };

    println!("{}", generator.secret()?);

    Ok(())
}
