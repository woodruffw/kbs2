use std::convert::TryInto;
use std::env;
use std::fmt::Write as _;
use std::io::{self, stdin, IsTerminal, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process;

use age::secrecy::SecretBox;
use anyhow::{anyhow, Result};
use arboard::Clipboard;
use clap::ArgMatches;
use daemonize::Daemonize;
use inquire::Confirm;
use nix::unistd::{fork, ForkResult};
use secrecy::ExposeSecret;

use crate::kbs2::agent;
use crate::kbs2::backend::{self, Backend};
use crate::kbs2::config::{self, Pinentry};
use crate::kbs2::generator::Generator;
use crate::kbs2::input::Input;
use crate::kbs2::record::{
    self, EnvironmentFields, LoginFields, Record, RecordBody, UnstructuredFields,
};
use crate::kbs2::session::Session;
use crate::kbs2::util;

/// Implements the `kbs2 init` command.
pub fn init(matches: &ArgMatches, config_dir: &Path) -> Result<()> {
    log::debug!("initializing a new config");

    #[allow(clippy::unwrap_used)]
    if config_dir.join(config::CONFIG_BASENAME).exists()
        && !*matches.get_one::<bool>("force").unwrap()
    {
        return Err(anyhow!(
            "refusing to overwrite your current config without --force"
        ));
    }

    #[allow(clippy::unwrap_used)]
    let store_dir = matches.get_one::<PathBuf>("store-dir").unwrap().as_path();

    // Warn, but don't fail, if the store directory is already present.
    if store_dir.exists() {
        util::warn("Requested store directory already exists");
    }

    #[allow(clippy::unwrap_used)]
    let password = if !*matches.get_one::<bool>("insecure-not-wrapped").unwrap() {
        Some(util::get_password(None, Pinentry::default())?)
    } else {
        None
    };

    config::initialize(&config_dir, &store_dir, password)
}

/// Implements the `kbs2 agent` command (and subcommands).
pub fn agent(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("agent subcommand dispatch");

    // No subcommand: run the agent itself
    if matches.subcommand().is_none() {
        let mut agent = agent::Agent::new()?;
        #[allow(clippy::unwrap_used)]
        if !matches.get_one::<bool>("foreground").unwrap() {
            Daemonize::new().start()?;
        }
        agent.run()?;
        return Ok(());
    }

    match matches.subcommand() {
        Some(("flush", matches)) => agent_flush(matches),
        Some(("query", matches)) => agent_query(matches, config),
        Some(("unwrap", matches)) => agent_unwrap(matches, config),
        _ => unreachable!(),
    }
}

/// Implements the `kbs2 agent flush` subcommand.
fn agent_flush(matches: &ArgMatches) -> Result<()> {
    log::debug!("asking the agent to flush all keys");

    let client = agent::Client::new()?;
    client.flush_keys()?;

    #[allow(clippy::unwrap_used)]
    if *matches.get_one::<bool>("quit").unwrap() {
        client.quit_agent()?;
    }

    Ok(())
}

/// Implements the `kbs2 agent query` subcommand.
fn agent_query(_matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("querying the agent for a key's existence");

    // It doesn't make sense to query the agent for keypairs that the agent
    // doesn't manage. Use a specific code to signal this case.
    if !config.wrapped {
        std::process::exit(2);
    }

    // Don't allow client creation to fail the normal way: if we can't create
    // a client for whatever reason (e.g., the agent isn't running), exit
    // with a specific code to signal our state to the user.
    let client = agent::Client::new().unwrap_or_else(|_| std::process::exit(3));
    if !client.query_key(&config.public_key)? {
        std::process::exit(1);
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
    if client.query_key(&config.public_key)? {
        println!("kbs2 agent already has this key; ignoring.");
        return Ok(());
    }

    let password = util::get_password(None, &config.pinentry)?;
    client.add_key(&config.public_key, &config.keyfile, password)?;

    Ok(())
}

/// Implements the `kbs2 new` command.
pub fn new(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("creating a new record");

    let session: Session = config.try_into()?;

    if let Some(pre_hook) = &session.config.commands.new.pre_hook {
        log::debug!("pre-hook: {}", pre_hook);
        session.config.call_hook(pre_hook, &[])?;
    }

    #[allow(clippy::unwrap_used)]
    let label = matches.get_one::<String>("label").unwrap();

    #[allow(clippy::unwrap_used)]
    if session.has_record(label) && !matches.get_one::<bool>("force").unwrap() {
        return Err(anyhow!("refusing to overwrite a record without --force"));
    }

    let config = session.config.with_matches(matches);

    #[allow(clippy::unwrap_used)]
    let record = match matches
        .get_one::<String>("kind")
        .map(AsRef::as_ref)
        .unwrap()
    {
        "login" => Record::new(label, LoginFields::input(&config)?),
        "environment" => Record::new(label, EnvironmentFields::input(&config)?),
        "unstructured" => Record::new(label, UnstructuredFields::input(&config)?),
        _ => unreachable!(),
    };

    session.add_record(&record)?;

    if let Some(post_hook) = &session.config.commands.new.post_hook {
        log::debug!("post-hook: {}", post_hook);
        session.config.call_hook(post_hook, &[label])?;
    }

    Ok(())
}

/// Implements the `kbs2 list` command.
pub fn list(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("listing records");

    let session: Session = config.try_into()?;

    #[allow(clippy::unwrap_used)]
    let (details, filter_kind) = (
        *matches.get_one::<bool>("details").unwrap(),
        matches.contains_id("kind"),
    );

    for label in session.record_labels()? {
        let mut display = String::new();

        if details || filter_kind {
            let record = session.get_record(&label)?;

            if filter_kind {
                #[allow(clippy::unwrap_used)]
                let kind = matches.get_one::<String>("kind").unwrap();
                if &record.body.to_string() != kind {
                    continue;
                }
            }

            display.push_str(&label);

            if details {
                write!(display, " {} {}", record.body, record.timestamp)?;
            }
        } else {
            display.push_str(&label);
        }

        println!("{display}");
    }

    Ok(())
}

/// Implements the `kbs2 rm` command.
pub fn rm(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("removing a record");

    let session: Session = config.try_into()?;

    #[allow(clippy::unwrap_used)]
    let labels: Vec<_> = matches
        .get_many::<String>("label")
        .unwrap()
        .map(AsRef::as_ref)
        .collect();

    for label in &labels {
        session.delete_record(label)?;
    }

    if let Some(post_hook) = &session.config.commands.rm.post_hook {
        log::debug!("post-hook: {}", post_hook);
        session.config.call_hook(post_hook, &labels)?;
    }

    Ok(())
}

/// Implements the `kbs2 rename` command.
pub fn rename(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("renaming a record");

    let session: Session = config.try_into()?;

    #[allow(clippy::unwrap_used)]
    let old_label: &str = matches.get_one::<String>("old-label").unwrap();

    #[allow(clippy::unwrap_used)]
    let new_label: &str = matches.get_one::<String>("new-label").unwrap();

    #[allow(clippy::unwrap_used)]
    if session.has_record(new_label) && !matches.get_one::<bool>("force").unwrap() {
        return Err(anyhow!("refusing to overwrite a record without --force"));
    }

    session.rename_record(old_label, new_label)?;

    if let Some(post_hook) = &session.config.commands.rename.post_hook {
        log::debug!("post-hook: {}", post_hook);
        session
            .config
            .call_hook(post_hook, &[old_label, new_label])?;
    }

    Ok(())
}

/// Implements the `kbs2 dump` command.
pub fn dump(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("dumping a record");

    let session: Session = config.try_into()?;

    #[allow(clippy::unwrap_used)]
    let labels: Vec<_> = matches.get_many::<String>("label").unwrap().collect();

    for label in labels {
        let record = session.get_record(label)?;

        #[allow(clippy::unwrap_used)]
        if *matches.get_one::<bool>("json").unwrap() {
            println!("{}", serde_json::to_string(&record)?);
        } else {
            println!("Label {}\nKind {}", label, record.body);

            match record.body {
                RecordBody::Login(l) => {
                    println!("Username {}\nPassword {}", l.username, l.password)
                }
                RecordBody::Environment(e) => {
                    println!("Variable {}\nValue {}", e.variable, e.value)
                }
                RecordBody::Unstructured(u) => println!("Contents {}", u.contents),
            }
        }
    }

    Ok(())
}

/// Implements the `kbs2 pass` command.
pub fn pass(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("getting a login's password");

    let session: Session = config.try_into()?;

    if let Some(pre_hook) = &session.config.commands.pass.pre_hook {
        log::debug!("pre-hook: {}", pre_hook);
        session.config.call_hook(pre_hook, &[])?;
    }

    #[allow(clippy::unwrap_used)]
    let label = matches.get_one::<String>("label").unwrap();
    let record = session.get_record(label)?;

    let login = match record.body {
        RecordBody::Login(l) => l,
        _ => return Err(anyhow!("not a login record: {}", label)),
    };

    let password = login.password;

    #[allow(clippy::unwrap_used)]
    if *matches.get_one::<bool>("clipboard").unwrap() {
        // NOTE(ww): fork() is unsafe in multithreaded programs where the child calls
        // non async-signal-safe functions. kbs2 is single threaded, so this usage is fine.
        unsafe {
            match fork() {
                Ok(ForkResult::Child) => {
                    clip(password, &session)?;
                }
                Err(_) => return Err(anyhow!("clipboard fork failed")),
                _ => {}
            }
        }
    } else if !stdin().is_terminal() {
        print!("{password}");
    } else {
        println!("{password}");
    }

    if let Some(post_hook) = &session.config.commands.pass.post_hook {
        log::debug!("post-hook: {}", post_hook);
        session.config.call_hook(post_hook, &[])?;
    }

    Ok(())
}

#[doc(hidden)]
fn clip(password: String, session: &Session) -> Result<()> {
    let clipboard_duration = session.config.commands.pass.clipboard_duration;
    let clear_after = session.config.commands.pass.clear_after;

    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(&password)?;

    std::thread::sleep(std::time::Duration::from_secs(clipboard_duration));

    if clear_after {
        clipboard.clear()?;

        if let Some(clear_hook) = &session.config.commands.pass.clear_hook {
            log::debug!("clear-hook: {}", clear_hook);
            session.config.call_hook(clear_hook, &[])?;
        }
    }

    Ok(())
}

/// Implements the `kbs2 env` command.
pub fn env(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("getting a environment variable");

    let session: Session = config.try_into()?;

    #[allow(clippy::unwrap_used)]
    let label = matches.get_one::<String>("label").unwrap();
    let record = session.get_record(label)?;

    let environment = match record.body {
        RecordBody::Environment(e) => e,
        _ => return Err(anyhow!("not an environment record: {}", label)),
    };

    #[allow(clippy::unwrap_used)]
    if *matches.get_one::<bool>("value-only").unwrap() {
        println!("{}", environment.value);
    } else if *matches.get_one::<bool>("no-export").unwrap() {
        println!("{}={}", environment.variable, environment.value);
    } else {
        println!("export {}={}", environment.variable, environment.value);
    }

    Ok(())
}

/// Implements the `kbs2 edit` command.
pub fn edit(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("editing a record");

    let session: Session = config.try_into()?;

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

    #[allow(clippy::unwrap_used)]
    let label = matches.get_one::<String>("label").unwrap();
    let record = session.get_record(label)?;

    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(&serde_json::to_vec_pretty(&record)?)?;

    if !process::Command::new(&editor)
        .args(&editor_args)
        .arg(file.path())
        .status()
        .map_or(false, |o| o.success())
    {
        return Err(anyhow!("failed to run the editor"));
    }

    // Rewind, pull the changed contents, deserialize back into a record.
    file.rewind()?;
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
pub fn generate(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    let generator = {
        #[allow(clippy::unwrap_used)]
        let generator_name = matches.get_one::<String>("generator").unwrap();
        match config.generator(generator_name) {
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

/// Implements the `kbs2 rewrap` command.
pub fn rewrap(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("attempting key rewrap");

    if !config.wrapped {
        return Err(anyhow!("config specifies a bare key; nothing to rewrap"));
    }

    #[allow(clippy::unwrap_used)]
    if !*matches.get_one::<bool>("no-backup").unwrap() {
        let keyfile_backup: PathBuf = format!("{}.old", &config.keyfile).into();

        #[allow(clippy::unwrap_used)]
        if keyfile_backup.exists() && !*matches.get_one::<bool>("force").unwrap() {
            return Err(anyhow!(
                "refusing to overwrite a previous key backup without --force"
            ));
        }

        std::fs::copy(&config.keyfile, &keyfile_backup)?;
        println!("Backup of the OLD wrapped keyfile saved to: {keyfile_backup:?}");
    }

    let old = util::get_password(Some("OLD master password: "), &config.pinentry)?;
    let new = util::get_password(Some("NEW master password: "), &config.pinentry)?;

    backend::RageLib::rewrap_keyfile(&config.keyfile, old, new)
}

/// Implements the `kbs2 rekey` command.
pub fn rekey(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("attempting to rekey the store");

    // This is an artificial limitation; bare keys should never be used outside of testing,
    // so support for them is unnecessary here.
    if !config.wrapped {
        return Err(anyhow!("rekeying is only supported on wrapped keys"));
    }

    let session: Session = config.try_into()?;

    println!(
        "This subcommand REKEYS your entire store ({}) and REWRITES your config",
        session.config.store
    );

    if !Confirm::new("Are you SURE you want to continue?")
        .with_default(false)
        .with_help_message("Be certain! If you are not certain, press [enter] to do nothing.")
        .prompt()?
    {
        return Ok(());
    }

    #[allow(clippy::unwrap_used)]
    if !*matches.get_one::<bool>("no-backup").unwrap() {
        // First, back up the keyfile.
        let keyfile_backup: PathBuf = format!("{}.old", &config.keyfile).into();
        if keyfile_backup.exists() {
            return Err(anyhow!(
                "refusing to overwrite a previous key backup during rekeying; resolve manually"
            ));
        }

        std::fs::copy(&config.keyfile, &keyfile_backup)?;
        println!("Backup of the OLD wrapped keyfile saved to: {keyfile_backup:?}");

        // Next, the config itself.
        let config_backup: PathBuf =
            Path::new(&config.config_dir).join(format!("{}.old", config::CONFIG_BASENAME));
        if config_backup.exists() {
            return Err(anyhow!(
                "refusing to overwrite a previous config backup during rekeying; resolve manually"
            ));
        }

        std::fs::copy(
            Path::new(&config.config_dir).join(config::CONFIG_BASENAME),
            &config_backup,
        )?;
        println!("Backup of the OLD config saved to: {config_backup:?}");

        // Finally, every record in the store.
        let store_backup: PathBuf = format!("{}.old", &config.store).into();
        if store_backup.exists() {
            return Err(anyhow!(
                "refusing to overwrite a previous store backup during rekeying; resolve manually"
            ));
        }

        std::fs::create_dir_all(&store_backup)?;
        for label in session.record_labels()? {
            std::fs::copy(
                Path::new(&config.store).join(&label),
                store_backup.join(&label),
            )?;
        }
        println!("Backup of the OLD store saved to: {:?}", &store_backup);
    }

    // Decrypt and collect all records.
    let records: Vec<SecretBox<record::Record>> = {
        let records: Result<Vec<record::Record>> = session
            .record_labels()?
            .iter()
            .map(|l| session.get_record(l))
            .collect();

        records?.into_iter().map(SecretBox::new).collect()
    };

    // Get a new master password.
    let new_password = util::get_password(Some("NEW master password: "), &config.pinentry)?;

    // Use it to generate a new wrapped keypair, overwriting the previous keypair.
    let public_key =
        backend::RageLib::create_wrapped_keypair(&config.keyfile, new_password.clone())?;

    // Dupe the current config, update only the public key field, and write it back.
    let config = config::Config {
        public_key,
        ..config.clone()
    };
    std::fs::write(
        Path::new(&config.config_dir).join(config::CONFIG_BASENAME),
        toml::to_string(&config)?,
    )?;

    // Flush the stale key from the active agent, and add the new key to the agent.
    // NOTE(ww): This scope is essential: we need to drop this client before we
    // create the new session below. Why? Because the session contains its
    // own agent client, and the current agent implementation only allows a
    // single client at a time. Clients yield their access by closing their
    // underlying socket, so we need to drop here to prevent a deadlock.
    {
        let client = agent::Client::new()?;
        client.flush_keys()?;
        client.add_key(&config.public_key, &config.keyfile, new_password)?;
    }

    // Create a new session from the new config and use it to re-encrypt each record.
    println!("Re-encrypting all records, be patient...");
    let session: Session = (&config).try_into()?;
    for record in records {
        log::debug!("re-encrypting {}", record.expose_secret().label);
        session.add_record(record.expose_secret())?;
    }

    println!("All done.");

    Ok(())
}

/// Implements the `kbs2 config` command.
pub fn config(matches: &ArgMatches, config: &config::Config) -> Result<()> {
    log::debug!("config subcommand dispatch");

    match matches.subcommand() {
        Some(("dump", matches)) =>
        {
            #[allow(clippy::unwrap_used)]
            if *matches.get_one::<bool>("pretty").unwrap() {
                serde_json::to_writer_pretty(io::stdout(), &config)?;
            } else {
                serde_json::to_writer(io::stdout(), &config)?;
            }
        }
        Some((_, _)) => unreachable!(),
        None => unreachable!(),
    }

    Ok(())
}
