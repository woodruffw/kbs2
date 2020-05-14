use clap::{App, AppSettings, Arg};

use std::path::Path;
use std::process;

mod kbs2;

fn app<'a, 'b>() -> App<'a, 'b> {
    // TODO(ww): Put this in a separate file, or switch to YAML.
    // The latter probably won't work with env!, though.
    App::new(env!("CARGO_PKG_NAME"))
        .setting(AppSettings::AllowExternalSubcommands)
        .version(env!("CARGO_PKG_VERSION"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::with_name("config")
                .help("use the specified config file")
                .short("c")
                .long("config")
                .value_name("FILE")
                .takes_value(true),
        )
        .subcommand(
            App::new("init")
                .about("initialize kbs2 with a new config")
                .arg(
                    Arg::with_name("force")
                        .help("overwrite, if already present")
                        .short("f")
                        .long("force"),
                )
                .arg(
                    Arg::with_name("keygen")
                        .help("generate a new key with the config")
                        .short("k")
                        .long("keygen"),
                ),
        )
        .subcommand(
            App::new("new")
                .about("create a new record")
                .arg(
                    Arg::with_name("force")
                        .help("overwrite, if already present")
                        .short("f")
                        .long("force"),
                )
                .arg(
                    Arg::with_name("kind")
                        .help("the kind of record to create")
                        .index(1)
                        .required(true)
                        .possible_values(kbs2::record::RECORD_KINDS),
                )
                .arg(
                    Arg::with_name("label")
                        .help("the record's label")
                        .index(2)
                        .required(true),
                ),
        )
        .subcommand(
            App::new("list")
                .about("list records")
                .arg(
                    Arg::with_name("details")
                        .help("print (non-field) details for each record")
                        .short("d")
                        .long("details"),
                )
                .arg(
                    Arg::with_name("kind")
                        .help("list only records of this kind")
                        .short("k")
                        .long("kind")
                        .takes_value(true)
                        .possible_values(kbs2::record::RECORD_KINDS),
                ),
        )
        .subcommand(
            App::new("rm").about("remove a record").arg(
                Arg::with_name("label")
                    .help("the record's label")
                    .index(1)
                    .required(true),
            ),
        )
        .subcommand(
            App::new("dump")
                .about("dump a record")
                .arg(
                    Arg::with_name("label")
                        .help("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::with_name("json")
                        .help("dump in JSON format")
                        .short("j")
                        .long("json"),
                ),
        )
        .subcommand(
            App::new("pass")
                .about("get the password in a login record")
                .arg(
                    Arg::with_name("label")
                        .help("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::with_name("clipboard")
                        .help("copy the password to the clipboard")
                        .short("c")
                        .long("clipboard"),
                ),
        )
        .subcommand(
            App::new("env")
                .about("get an environment record")
                .arg(
                    Arg::with_name("label")
                        .help("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::with_name("value-only")
                        .help("print only the environment variable value, not the variable name")
                        .short("v")
                        .long("--value-only"),
                )
                .arg(
                    Arg::with_name("no-export")
                        .help("print only VAR=val without `export`")
                        .short("-n")
                        .long("--no-export"),
                ),
        )
}

fn run() -> Result<(), kbs2::error::Error> {
    let matches = app().get_matches();

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

        match matches.subcommand() {
            ("new", Some(matches)) => kbs2::command::new(&matches, config),
            ("list", Some(matches)) => kbs2::command::list(&matches, config),
            ("rm", Some(matches)) => kbs2::command::rm(&matches, config),
            ("dump", Some(matches)) => kbs2::command::dump(&matches, config),
            ("pass", Some(matches)) => kbs2::command::pass(&matches, config),
            ("env", Some(matches)) => kbs2::command::env(&matches, config),
            (cmd, Some(matches)) => {
                let ext_args: Vec<&str> = match matches.values_of("") {
                    Some(values) => values.collect(),
                    None => vec![],
                };

                log::debug!("external command requested: {} (args: {:?})", cmd, ext_args);

                match kbs2::util::run_with_status(cmd, &ext_args) {
                    Some(true) => Ok(()),
                    Some(false) => process::exit(2),
                    None => Err(format!("no such command: {}", cmd).into()),
                }
            }
            ("", None) => Ok(println!(
                "{}\n\nSee --help for more information.",
                matches.usage()
            )),
            _ => unreachable!(),
        }
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
