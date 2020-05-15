use clap::{App, AppSettings, Arg};
use clap_generate::{generate, generators};

use std::io;
use std::path::Path;
use std::process;

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
        .subcommand(
            App::new("new")
                .about("create a new record")
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
                        .long("--value-only"),
                )
                .arg(
                    Arg::with_name("no-export")
                        .about("print only VAR=val without `export`")
                        .short('n')
                        .long("--no-export"),
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

    // `init` is a special case, since it doesn't have access to a preexisting config.
    if let ("init", Some(matches)) = matches.subcommand() {
        kbs2::command::init(&matches, &config_dir)
    } else {
        let config = kbs2::config::load(&config_dir)?;
        log::debug!("loaded config: {:?}", config);

        let session = kbs2::session::Session::new(config);

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
            (cmd, Some(matches)) => {
                let cmd = format!("kbs2-{}", cmd);

                let ext_args: Vec<&str> = match matches.values_of("") {
                    Some(values) => values.collect(),
                    None => vec![],
                };

                log::debug!("external command requested: {} (args: {:?})", cmd, ext_args);

                match kbs2::util::run_with_status(&cmd, &ext_args) {
                    Some(true) => (),
                    Some(false) => process::exit(2),
                    None => return Err(format!("no such command: {}", cmd).into()),
                }
            }
            ("", None) => {
                return app
                    .clone()
                    .write_long_help(&mut io::stdout())
                    .map_err(|_| "failed to print help".into())
            }
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
