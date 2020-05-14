use atty::Stream;
use clap::ArgMatches;
use clipboard::{ClipboardContext, ClipboardProvider};
use inflector::Inflector;
use nix::unistd::{fork, ForkResult};

use std::path::Path;

use crate::kbs2::config;
use crate::kbs2::error::Error;
use crate::kbs2::input;
use crate::kbs2::record;
use crate::kbs2::session;
use crate::kbs2::util;

pub fn init(matches: &ArgMatches, config_dir: &Path) -> Result<(), Error> {
    log::debug!("initializing a new config");

    if config_dir.join(config::CONFIG_BASENAME).exists() && !matches.is_present("force") {
        return Err("refusing to overwrite your current config without --force".into());
    }

    config::initialize(&config_dir)
}

pub fn new(matches: &ArgMatches, config: config::Config) -> Result<(), Error> {
    log::debug!("creating a new record");

    let session = session::Session::new(config);

    if let Some(pre_hook) = &session.config.commands.new.pre_hook {
        log::debug!("pre-hook: {}", pre_hook);
        if !util::run_with_status(pre_hook, &[]).unwrap_or(false) {
            return Err(format!("pre-hook failed: {}", pre_hook).into());
        }
    }

    let label = matches.value_of("label").unwrap();
    if session.has_record(label) && !matches.is_present("force") {
        return Err("refusing to overwrite a record without --force".into());
    }

    let terse = atty::isnt(Stream::Stdin) || matches.is_present("terse");

    // TODO: new_* below is a little silly. This should be de-duped.
    match matches.value_of("kind").unwrap() {
        "login" => new_login(label, terse, &session)?,
        "environment" => new_environment(label, terse, &session)?,
        "unstructured" => new_unstructured(label, terse, &session)?,
        _ => unreachable!(),
    }

    if let Some(post_hook) = &session.config.commands.new.post_hook {
        log::debug!("post-hook: {}", post_hook);
        if !util::run_with_status(post_hook, &[&label]).unwrap_or(false) {
            // NOTE(ww): Maybe make this a warning instead?
            return Err(format!("post-hook failed: {}", post_hook).into());
        }
    }

    Ok(())
}

fn new_login(label: &str, terse: bool, session: &session::Session) -> Result<(), Error> {
    // TODO(ww): Passing whether or not a field is sensitive with this tuple is ugly.
    // We should really do something like Insensitive("username"), Sensitive("password"), etc.
    let fields = input::fields(&[("Username", false), ("Password", true)], terse)?;
    let record = record::Record::login(label, &fields[0], &fields[1]);

    session.add_record(&record)
}

fn new_environment(label: &str, terse: bool, session: &session::Session) -> Result<(), Error> {
    let fields = input::fields(&[("Variable", false), ("Value", true)], terse)?;
    let record = record::Record::environment(label, &fields[0], &fields[1]);

    session.add_record(&record)
}

fn new_unstructured(label: &str, terse: bool, session: &session::Session) -> Result<(), Error> {
    let fields = input::fields(&[("Contents", false)], terse)?;
    let record = record::Record::unstructured(label, &fields[0]);

    session.add_record(&record)
}

pub fn list(matches: &ArgMatches, config: config::Config) -> Result<(), Error> {
    log::debug!("listing records");

    let (details, filter_kind) = (matches.is_present("details"), matches.is_present("kind"));
    let session = session::Session::new(config);

    for label in session.record_labels()? {
        let mut display = String::new();

        if details || filter_kind {
            let record = session.get_record(&label)?;

            if filter_kind {
                let kind = matches.value_of("kind").unwrap();
                if record.kind.to_string() != kind {
                    continue;
                }
            }

            display.push_str(&label);

            if details {
                display.push_str(&format!(
                    "\n\tKind: {}\n\tTimestamp: {}",
                    record.kind, record.timestamp
                ));
            }
        } else {
            display.push_str(&label);
        }

        println!("{}", display);
    }

    Ok(())
}

pub fn rm(matches: &ArgMatches, config: config::Config) -> Result<(), Error> {
    log::debug!("removing a record");

    let session = session::Session::new(config);
    session.delete_record(matches.value_of("label").unwrap())
}

pub fn dump(matches: &ArgMatches, config: config::Config) -> Result<(), Error> {
    log::debug!("dumping a record");

    let session = session::Session::new(config);
    let label = matches.value_of("label").unwrap();
    let record = session.get_record(&label)?;

    if matches.is_present("json") {
        println!("{}", serde_json::to_string(&record)?);
    } else {
        println!("Label: {}\n\tKind: {}", label, record.kind.to_string());

        for field in record.fields {
            println!("\t{}: {}", field.name.to_sentence_case(), field.value);
        }
    }

    Ok(())
}

pub fn pass(matches: &ArgMatches, config: config::Config) -> Result<(), Error> {
    log::debug!("getting a login's password");

    let session = session::Session::new(config);
    let label = matches.value_of("label").unwrap();
    let record = session.get_record(&label)?;

    if record.kind != record::RecordKind::Login {
        return Err(format!("not a login record: {}", label).into());
    }

    let password = record.get_expected_field("password")?;
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
                let mut ctx: ClipboardContext =
                    ClipboardProvider::new().map_err(|_| "unable to grab the clipboard")?;

                ctx.set_contents(password.to_owned())
                    .map_err(|_| "unable to store to the clipboard")?;

                std::thread::sleep(std::time::Duration::from_secs(clipboard_duration));

                if clear_after {
                    ctx.set_contents("".to_owned())
                        .map_err(|_| "unable to clear the clipboard")?;
                }
            }
            Err(_) => return Err("clipboard fork failed".into()),
            _ => {}
        }
    } else {
        println!("{}", password);
    }

    Ok(())
}

pub fn env(matches: &ArgMatches, config: config::Config) -> Result<(), Error> {
    log::debug!("getting a environment variable");

    let session = session::Session::new(config);
    let label = matches.value_of("label").unwrap();
    let record = session.get_record(&label)?;

    if record.kind != record::RecordKind::Environment {
        return Err(format!("not a environment record: {}", label).into());
    }

    let value = record.get_expected_field("value")?;
    if matches.is_present("value-only") {
        println!("{}", value);
    } else {
        let variable = record.get_expected_field("variable")?;
        if matches.is_present("no-export") {
            println!("{}={}", variable, value);
        } else {
            println!("export {}={}", variable, value);
        }
    }

    Ok(())
}
