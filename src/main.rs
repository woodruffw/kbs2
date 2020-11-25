use anyhow::{anyhow, Result};
use clap::{App, AppSettings, Arg};
use clap_generate::{generate, generators};

use std::io;
use std::path::Path;
use std::process::{self, Command};

mod kbs2;

fn app<'a>() -> App<'a> {
    // TODO(ww): Put this in a separate file, or switch to YAML.
    // The latter probably won't work with env!, though.
    App::new(env!("CARGO_PKG_NAME"))
        .setting(AppSettings::AllowExternalSubcommands)
        .setting(AppSettings::VersionlessSubcommands)
        .version(env!("CARGO_PKG_VERSION"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::new("config-dir")
                .about("use the specified config directory")
                .short('c')
                .long("config-dir")
                .value_name("DIR")
                .takes_value(true)
                .env("KBS2_CONFIG_DIR"),
        )
        .arg(
            Arg::new("completions")
                .about("emit shell tab completions")
                .long("completions")
                .value_name("SHELL")
                .takes_value(true)
                .possible_values(&["bash", "zsh", "fish"]),
        )
        .subcommand(
            App::new("agent")
                .about("run the kbs2 authentication agent")
                .arg(
                    Arg::new("foreground")
                        .about("run the agent in the foreground")
                        .short('F')
                        .long("foreground"),
                )
                .arg(
                    Arg::new("no-unwrap")
                        .about("don't unwrap the current config's key")
                        .short('U')
                        .long("no-unwrap"),
                )
                .subcommand(
                    App::new("flush")
                        .about("remove all unwrapped keys from the running agent")
                        .arg(
                            Arg::new("quit")
                                .about("quit the agent after flushing")
                                .short('q')
                                .long("quit"),
                        ),
                )
                .subcommand(
                    App::new("unwrap")
                        .about("unwrap the current config's key in the running agent"),
                ),
        )
        .subcommand(
            App::new("init")
                .about("initialize kbs2 with a new config and keypair")
                .arg(
                    Arg::new("force")
                        .about("overwrite the config and keyfile, if already present")
                        .short('f')
                        .long("force"),
                )
                .arg(
                    Arg::new("insecure-not-wrapped")
                        .about("don't wrap the keypair with a master password")
                        .long("insecure-not-wrapped"),
                ),
        )
        .subcommand(
            App::new("new")
                .about("create a new record")
                .arg(
                    Arg::new("label")
                        .about("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::new("kind")
                        .about("the kind of record to create")
                        .short('k')
                        .long("kind")
                        .takes_value(true)
                        .possible_values(kbs2::record::RECORD_KINDS)
                        .default_value("login"),
                )
                .arg(
                    Arg::new("force")
                        .about("overwrite, if already present")
                        .short('f')
                        .long("force"),
                )
                .arg(
                    Arg::new("terse")
                        .about("read fields in a terse format, even when connected to a tty")
                        .short('t')
                        .long("terse"),
                )
                .arg(
                    Arg::new("generate")
                        .about("generate sensitive fields instead of prompting for them")
                        .short('g')
                        .long("generate"),
                )
                .arg(
                    Arg::new("generator")
                        .about("use the given generator to generate sensitive fields")
                        .short('G')
                        .long("generator")
                        .takes_value(true)
                        .default_value("default"),
                ),
        )
        .subcommand(
            App::new("list")
                .about("list records")
                .arg(
                    Arg::new("details")
                        .about("print (non-field) details for each record")
                        .short('d')
                        .long("details"),
                )
                .arg(
                    Arg::new("kind")
                        .about("list only records of this kind")
                        .short('k')
                        .long("kind")
                        .takes_value(true)
                        .possible_values(kbs2::record::RECORD_KINDS),
                ),
        )
        .subcommand(
            App::new("rm").about("remove a record").arg(
                Arg::new("label")
                    .about("the record's label")
                    .index(1)
                    .required(true),
            ),
        )
        .subcommand(
            App::new("dump")
                .about("dump a record")
                .arg(
                    Arg::new("label")
                        .about("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::new("json")
                        .about("dump in JSON format")
                        .short('j')
                        .long("json"),
                ),
        )
        .subcommand(
            App::new("pass")
                .about("get the password in a login record")
                .arg(
                    Arg::new("label")
                        .about("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::new("clipboard")
                        .about("copy the password to the clipboard")
                        .short('c')
                        .long("clipboard"),
                ),
        )
        .subcommand(
            App::new("env")
                .about("get an environment record")
                .arg(
                    Arg::new("label")
                        .about("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::new("value-only")
                        .about("print only the environment variable value, not the variable name")
                        .short('v')
                        .long("value-only"),
                )
                .arg(
                    Arg::new("no-export")
                        .about("print only VAR=val without `export`")
                        .short('n')
                        .long("no-export"),
                ),
        )
        .subcommand(
            App::new("edit")
                .about("modify a record with a text editor")
                .arg(
                    Arg::new("label")
                        .about("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::new("preserve-timestamp")
                        .about("don't update the record's timestamp")
                        .short('p')
                        .long("preserve-timestamp"),
                ),
        )
        .subcommand(
            App::new("generate")
                .about("generate secret values using a generator")
                .arg(
                    Arg::new("generator")
                        .about("the generator to use")
                        .index(1)
                        .default_value("default"),
                ),
        )
}

fn run() -> Result<()> {
    let mut app = app();
    let matches = app.clone().get_matches();

    if let Some(shell) = matches.value_of("completions") {
        match shell {
            "bash" => {
                generate::<generators::Bash, _>(&mut app, env!("CARGO_PKG_NAME"), &mut io::stdout())
            }
            "zsh" => {
                generate::<generators::Zsh, _>(&mut app, env!("CARGO_PKG_NAME"), &mut io::stdout())
            }
            "fish" => {
                generate::<generators::Fish, _>(&mut app, env!("CARGO_PKG_NAME"), &mut io::stdout())
            }
            _ => unreachable!(),
        }
        return Ok(());
    }

    let config_dir = match matches.value_of("config-dir") {
        Some(path) => Path::new(path).to_path_buf(),
        None => kbs2::config::find_config_dir()?,
    };

    log::debug!("config dir: {:?}", config_dir);
    std::fs::create_dir_all(&config_dir)?;

    // Subcommand dispatch happens here. All subcommands take a `Config`, with two exceptions:
    //
    // * No subcommand (i.e., just `kbs2`) does nothing besides printing help.
    //
    // * `kbs2 init` doesn't have access to a preexisting config, and so needs to be separated
    //   from the config-loading behavior of all other subcommands.
    //
    // Internally, most (but not all) subcommands load a `Session` from their borrowed
    // `Config` argument. This `Session` is in turn used to perform record and encryption
    // operations.
    if matches.subcommand().is_none() {
        app.clone()
            .write_long_help(&mut io::stdout())
            .map_err(|_| anyhow!("failed to print help"))
    } else if let Some(("init", matches)) = matches.subcommand() {
        kbs2::command::init(&matches, &config_dir)
    } else {
        let config = kbs2::config::load(&config_dir)?;
        log::debug!("loaded config: {:?}", config);

        if let Some(("agent", matches)) = matches.subcommand() {
            return kbs2::command::agent(&matches, &config);
        }

        if let Some(pre_hook) = &config.pre_hook {
            log::debug!("pre-hook: {}", pre_hook);
            config.call_hook(pre_hook, &[])?;
        }

        match matches.subcommand() {
            Some(("new", matches)) => kbs2::command::new(&matches, &config)?,
            Some(("list", matches)) => kbs2::command::list(&matches, &config)?,
            Some(("rm", matches)) => kbs2::command::rm(&matches, &config)?,
            Some(("dump", matches)) => kbs2::command::dump(&matches, &config)?,
            Some(("pass", matches)) => kbs2::command::pass(&matches, &config)?,
            Some(("env", matches)) => kbs2::command::env(&matches, &config)?,
            Some(("edit", matches)) => kbs2::command::edit(&matches, &config)?,
            Some(("generate", matches)) => kbs2::command::generate(&matches, &config)?,
            Some((cmd, matches)) => {
                let cmd = format!("kbs2-{}", cmd);

                let ext_args: Vec<&str> = match matches.values_of("") {
                    Some(values) => values.collect(),
                    None => vec![],
                };

                log::debug!("external command requested: {} (args: {:?})", cmd, ext_args);

                let status = Command::new(&cmd)
                    .args(&ext_args)
                    .env("KBS2_CONFIG_DIR", &config_dir)
                    .env("KBS2_STORE", &config.store)
                    .env("KBS2_SUBCOMMAND", "1")
                    .status()
                    .map_or(None, |s| Some(s.success()));

                match status {
                    Some(true) => (),
                    Some(false) => process::exit(2),
                    None => return Err(anyhow!("no such command: {}", cmd)),
                }
            }
            _ => unreachable!(),
        }

        if let Some(post_hook) = &config.post_hook {
            log::debug!("post-hook: {}", post_hook);
            config.call_hook(post_hook, &[])?;
        }

        Ok(())
    }
}

fn main() -> Result<()> {
    env_logger::init();
    run()
}
