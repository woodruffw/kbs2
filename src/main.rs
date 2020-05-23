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
        .version(env!("CARGO_PKG_VERSION"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::with_name("config")
                .about("use the specified config file")
                .short('c')
                .long("config")
                .value_name("FILE")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("completions")
                .about("emit shell tab completions")
                .long("completions")
                .value_name("SHELL")
                .takes_value(true)
                .possible_values(&["bash", "zsh", "fish"]),
        )
        .subcommand(
            App::new("init")
                .about("initialize kbs2 with a new config")
                .arg(
                    Arg::with_name("force")
                        .about("overwrite, if already present")
                        .short('f')
                        .long("force"),
                )
                .arg(
                    Arg::with_name("keygen")
                        .about("generate a new key with the config")
                        .short('k')
                        .long("keygen"),
                ),
        )
        .subcommand(App::new("unlock").about("unwrap the private key for use"))
        .subcommand(App::new("lock").about("remove the unwrapped key, if any, from shared memory"))
        .subcommand(
            App::new("new")
                .about("create a new record")
                .arg(
                    Arg::with_name("kind")
                        .about("the kind of record to create")
                        .index(1)
                        .required(true)
                        .possible_values(kbs2::record::RECORD_KINDS),
                )
                .arg(
                    Arg::with_name("label")
                        .about("the record's label")
                        .index(2)
                        .required(true),
                )
                .arg(
                    Arg::with_name("force")
                        .about("overwrite, if already present")
                        .short('f')
                        .long("force"),
                )
                .arg(
                    Arg::with_name("terse")
                        .about("read fields in a terse format, even when connected to a tty")
                        .short('t')
                        .long("terse"),
                )
                .arg(
                    Arg::with_name("generate")
                        .about("generate sensitive fields instead of prompting for them")
                        .short('g')
                        .long("generate"),
                )
                .arg(
                    Arg::with_name("generator")
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
                    Arg::with_name("details")
                        .about("print (non-field) details for each record")
                        .short('d')
                        .long("details"),
                )
                .arg(
                    Arg::with_name("kind")
                        .about("list only records of this kind")
                        .short('k')
                        .long("kind")
                        .takes_value(true)
                        .possible_values(kbs2::record::RECORD_KINDS),
                ),
        )
        .subcommand(
            App::new("rm").about("remove a record").arg(
                Arg::with_name("label")
                    .about("the record's label")
                    .index(1)
                    .required(true),
            ),
        )
        .subcommand(
            App::new("dump")
                .about("dump a record")
                .arg(
                    Arg::with_name("label")
                        .about("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::with_name("json")
                        .about("dump in JSON format")
                        .short('j')
                        .long("json"),
                ),
        )
        .subcommand(
            App::new("pass")
                .about("get the password in a login record")
                .arg(
                    Arg::with_name("label")
                        .about("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::with_name("clipboard")
                        .about("copy the password to the clipboard")
                        .short('c')
                        .long("clipboard"),
                ),
        )
        .subcommand(
            App::new("env")
                .about("get an environment record")
                .arg(
                    Arg::with_name("label")
                        .about("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::with_name("value-only")
                        .about("print only the environment variable value, not the variable name")
                        .short('v')
                        .long("value-only"),
                )
                .arg(
                    Arg::with_name("no-export")
                        .about("print only VAR=val without `export`")
                        .short('n')
                        .long("no-export"),
                ),
        )
        .subcommand(
            App::new("edit")
                .about("modify a record with a text editor")
                .arg(
                    Arg::with_name("label")
                        .about("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::with_name("preserve-timestamp")
                        .about("don't update the record's timestamp")
                        .short('p')
                        .long("preserve-timestamp"),
                ),
        )
}

fn run() -> Result<(), kbs2::error::Error> {
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

    let config_dir = match matches.value_of("config") {
        Some(path) => Path::new(path).to_path_buf(),
        None => kbs2::config::find_config_dir()?,
    };

    log::debug!("config dir: {:?}", config_dir);
    std::fs::create_dir_all(&config_dir)?;

    // Subcommand dispatch happens here. All subcommands take a `Session`, with three exceptions:
    //
    // * `kbs2 init` doesn't have access to a preexisting config, and so needs to be separated
    //   from the config-loading behavior of all other subcommands.
    //
    // * `kbs2 unlock` exists so that all commands that make use of a session don't have to
    //   prompt for the master password themselves. That means that it can't take a session of
    //   its own.
    //
    // * `kbs2 lock` exists to remove the shared memory object created by `kbs2 unlock`. Taking
    //   a session would mean that it would attempt to pointlessly unlock the key before re-locking.
    if let ("init", Some(matches)) = matches.subcommand() {
        kbs2::command::init(&matches, &config_dir)
    } else if let ("unlock", Some(matches)) = matches.subcommand() {
        let config = kbs2::config::load(&config_dir)?;
        kbs2::command::unlock(&matches, &config)
    } else if let ("lock", Some(matches)) = matches.subcommand() {
        let config = kbs2::config::load(&config_dir)?;
        kbs2::command::lock(&matches, &config)
    } else {
        let config = kbs2::config::load(&config_dir)?;
        log::debug!("loaded config: {:?}", config);

        let session = kbs2::session::Session::new(config)?;

        if let Some(pre_hook) = &session.config.pre_hook {
            log::debug!("pre-hook: {}", pre_hook);
            session.config.call_hook(pre_hook, &[])?;
        }

        match matches.subcommand() {
            ("new", Some(matches)) => kbs2::command::new(&matches, &session)?,
            ("list", Some(matches)) => kbs2::command::list(&matches, &session)?,
            ("rm", Some(matches)) => kbs2::command::rm(&matches, &session)?,
            ("dump", Some(matches)) => kbs2::command::dump(&matches, &session)?,
            ("pass", Some(matches)) => kbs2::command::pass(&matches, &session)?,
            ("env", Some(matches)) => kbs2::command::env(&matches, &session)?,
            ("edit", Some(matches)) => kbs2::command::edit(&matches, &session)?,
            (cmd, Some(matches)) => {
                let cmd = format!("kbs2-{}", cmd);

                let ext_args: Vec<&str> = match matches.values_of("") {
                    Some(values) => values.collect(),
                    None => vec![],
                };

                log::debug!("external command requested: {} (args: {:?})", cmd, ext_args);

                let status = Command::new(&cmd)
                    .args(&ext_args)
                    .env("KBS2_STORE", &session.config.store)
                    .env("KBS2_SUBCOMMAND", "1")
                    .status()
                    .map_or(None, |s| Some(s.success()));

                match status {
                    Some(true) => (),
                    Some(false) => process::exit(2),
                    None => return Err(format!("no such command: {}", cmd).into()),
                }
            }
            ("", None) => app
                .clone()
                .write_long_help(&mut io::stdout())
                .map_err(|_| "failed to print help")?,
            _ => unreachable!(),
        }

        if let Some(post_hook) = &session.config.post_hook {
            log::debug!("post-hook: {}", post_hook);
            session.config.call_hook(post_hook, &[])?;
        }

        Ok(())
    }
}

fn main() {
    env_logger::init();

    process::exit(match run() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Fatal: {}", e);
            1
        }
    });
}
