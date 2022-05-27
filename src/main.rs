use std::io;
use std::path::Path;
use std::process;

use anyhow::{anyhow, Context, Result};
use clap::{Arg, ArgMatches, Command, ValueHint};
use clap_complete::{generate, Shell};

mod kbs2;

fn app() -> Command<'static> {
    // TODO(ww): Put this in a separate file, or switch to YAML.
    // The latter probably won't work with env!, though.
    Command::new(env!("CARGO_PKG_NAME"))
        .allow_external_subcommands(true)
        .version(env!("KBS2_BUILD_VERSION"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::new("config-dir")
                .help("use the specified config directory")
                .short('c')
                .long("config-dir")
                .value_name("DIR")
                .takes_value(true)
                .allow_invalid_utf8(true)
                .env("KBS2_CONFIG_DIR")
                .default_value_os(kbs2::config::DEFAULT_CONFIG_DIR.as_ref())
                .value_hint(ValueHint::DirPath),
        )
        .arg(
            Arg::new("completions")
                .help("emit shell tab completions")
                .long("completions")
                .value_name("SHELL")
                .takes_value(true)
                .possible_values(Shell::possible_values()),
        )
        .subcommand(
            Command::new("agent")
                .about("run the kbs2 authentication agent")
                .arg(
                    Arg::new("foreground")
                        .help("run the agent in the foreground")
                        .short('F')
                        .long("foreground"),
                )
                .subcommand(
                    Command::new("flush")
                        .about("remove all unwrapped keys from the running agent")
                        .arg(
                            Arg::new("quit")
                                .help("quit the agent after flushing")
                                .short('q')
                                .long("quit"),
                        ),
                )
                .subcommand(
                    Command::new("query")
                        .about("ask the current agent whether it has the current config's key"),
                )
                .subcommand(
                    Command::new("unwrap")
                        .about("unwrap the current config's key in the running agent"),
                ),
        )
        .subcommand(
            Command::new("init")
                .about("initialize kbs2 with a new config and keypair")
                .arg(
                    Arg::new("force")
                        .help("overwrite the config and keyfile, if already present")
                        .short('f')
                        .long("force"),
                )
                .arg(
                    Arg::new("store-dir")
                        .help("the directory to store encrypted kbs2 records in")
                        .short('s')
                        .long("store-dir")
                        .value_name("DIR")
                        .takes_value(true)
                        .allow_invalid_utf8(true)
                        .default_value_os(kbs2::config::DEFAULT_STORE_DIR.as_ref())
                        .value_hint(ValueHint::DirPath),
                )
                .arg(
                    Arg::new("insecure-not-wrapped")
                        .help("don't wrap the keypair with a master password")
                        .long("insecure-not-wrapped"),
                ),
        )
        .subcommand(
            Command::new("new")
                .about("create a new record")
                .arg(
                    Arg::new("label")
                        .help("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::new("kind")
                        .help("the kind of record to create")
                        .short('k')
                        .long("kind")
                        .takes_value(true)
                        .possible_values(kbs2::record::RECORD_KINDS)
                        .default_value("login"),
                )
                .arg(
                    Arg::new("force")
                        .help("overwrite, if already present")
                        .short('f')
                        .long("force"),
                )
                .arg(
                    Arg::new("terse")
                        .help("read fields in a terse format, even when connected to a tty")
                        .short('t')
                        .long("terse"),
                )
                .arg(
                    Arg::new("generator")
                        .help("use the given generator to generate sensitive fields")
                        .short('G')
                        .long("generator")
                        .takes_value(true)
                        .default_value("default"),
                ),
        )
        .subcommand(
            Command::new("list")
                .about("list records")
                .arg(
                    Arg::new("details")
                        .help("print (non-field) details for each record")
                        .short('d')
                        .long("details"),
                )
                .arg(
                    Arg::new("kind")
                        .help("list only records of this kind")
                        .short('k')
                        .long("kind")
                        .takes_value(true)
                        .possible_values(kbs2::record::RECORD_KINDS),
                ),
        )
        .subcommand(
            Command::new("rm").about("remove one or more records").arg(
                Arg::new("label")
                    .help("the labels of the records to remove")
                    .index(1)
                    .required(true)
                    .multiple_values(true),
            ),
        )
        .subcommand(
            Command::new("dump")
                .about("dump one or more records")
                .arg(
                    Arg::new("label")
                        .help("the labels of the records to dump")
                        .index(1)
                        .required(true)
                        .multiple_values(true),
                )
                .arg(
                    Arg::new("json")
                        .help("dump in JSON format (JSONL when multiple)")
                        .short('j')
                        .long("json"),
                ),
        )
        .subcommand(
            Command::new("pass")
                .about("get the password in a login record")
                .arg(
                    Arg::new("label")
                        .help("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::new("clipboard")
                        .help("copy the password to the clipboard")
                        .short('c')
                        .long("clipboard"),
                ),
        )
        .subcommand(
            Command::new("env")
                .about("get an environment record")
                .arg(
                    Arg::new("label")
                        .help("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::new("value-only")
                        .help("print only the environment variable value, not the variable name")
                        .short('v')
                        .long("value-only"),
                )
                .arg(
                    Arg::new("no-export")
                        .help("print only VAR=val without `export`")
                        .short('n')
                        .long("no-export"),
                ),
        )
        .subcommand(
            Command::new("edit")
                .about("modify a record with a text editor")
                .arg(
                    Arg::new("label")
                        .help("the record's label")
                        .index(1)
                        .required(true),
                )
                .arg(
                    Arg::new("preserve-timestamp")
                        .help("don't update the record's timestamp")
                        .short('p')
                        .long("preserve-timestamp"),
                ),
        )
        .subcommand(
            Command::new("generate")
                .about("generate secret values using a generator")
                .arg(
                    Arg::new("generator")
                        .help("the generator to use")
                        .index(1)
                        .default_value("default"),
                ),
        )
        .subcommand(
            Command::new("rewrap")
                .about("change the master password on a wrapped key")
                .arg(
                    Arg::new("no-backup")
                        .help("don't make a backup of the old wrapped key")
                        .short('n')
                        .long("no-backup"),
                )
                .arg(
                    Arg::new("force")
                        .help("overwrite a previous backup, if one exists")
                        .short('f')
                        .long("force"),
                ),
        )
        .subcommand(
            // NOTE: The absence of a --force option here is intentional.
            Command::new("rekey")
                .about("re-encrypt the entire store with a new keypair and master password")
                .arg(
                    Arg::new("no-backup")
                        .help("don't make a backup of the old wrapped key, config, or store")
                        .short('n')
                        .long("no-backup"),
                ),
        )
        .subcommand(
            Command::new("config")
                .subcommand_required(true)
                .about("interact with kbs2's configuration file")
                .subcommand(
                    Command::new("dump")
                        .about("dump the active configuration file as JSON")
                        .arg(
                            Arg::new("pretty")
                                .help("pretty-print the JSON")
                                .short('p')
                                .long("pretty"),
                        ),
                ),
        )
}

fn run(matches: &ArgMatches, config: &kbs2::config::Config) -> Result<()> {
    // Subcommand dispatch happens here. All subcommands handled here take a `Config`.
    //
    // Internally, most (but not all) subcommands load a `Session` from their borrowed
    // `Config` argument. This `Session` is in turn used to perform record and encryption
    // operations.

    // Special case: `kbs2 agent` does not receive pre- or post-hooks.
    if let Some(("agent", matches)) = matches.subcommand() {
        return kbs2::command::agent(matches, config);
    }

    if let Some(pre_hook) = &config.pre_hook {
        log::debug!("pre-hook: {}", pre_hook);
        config.call_hook(pre_hook, &[])?;
    }

    match matches.subcommand() {
        Some(("new", matches)) => kbs2::command::new(matches, config)?,
        Some(("list", matches)) => kbs2::command::list(matches, config)?,
        Some(("rm", matches)) => kbs2::command::rm(matches, config)?,
        Some(("dump", matches)) => kbs2::command::dump(matches, config)?,
        Some(("pass", matches)) => kbs2::command::pass(matches, config)?,
        Some(("env", matches)) => kbs2::command::env(matches, config)?,
        Some(("edit", matches)) => kbs2::command::edit(matches, config)?,
        Some(("generate", matches)) => kbs2::command::generate(matches, config)?,
        Some(("rewrap", matches)) => kbs2::command::rewrap(matches, config)?,
        Some(("rekey", matches)) => kbs2::command::rekey(matches, config)?,
        Some(("config", matches)) => kbs2::command::config(matches, config)?,
        Some((cmd, matches)) => {
            let cmd = format!("kbs2-{}", cmd);

            let ext_args: Vec<&str> = match matches.values_of("") {
                Some(values) => values.collect(),
                None => vec![],
            };

            log::debug!("external command requested: {} (args: {:?})", cmd, ext_args);

            let status = process::Command::new(&cmd)
                .args(&ext_args)
                .env("KBS2_CONFIG_DIR", &config.config_dir)
                .env("KBS2_STORE", &config.store)
                .env("KBS2_SUBCOMMAND", "1")
                .env("KBS2_MAJOR_VERSION", env!("CARGO_PKG_VERSION_MAJOR"))
                .env("KBS2_MINOR_VERSION", env!("CARGO_PKG_VERSION_MINOR"))
                .env("KBS2_PATCH_VERSION", env!("CARGO_PKG_VERSION_PATCH"))
                .status()
                .with_context(|| format!("no such command: {}", cmd))?;

            if !status.success() {
                return Err(match status.code() {
                    Some(code) => anyhow!("{} failed: exited with {}", cmd, code),
                    None => anyhow!("{} failed: terminated by signal", cmd),
                });
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

fn main() -> Result<()> {
    env_logger::init();

    let mut app = app();
    let matches = app.clone().get_matches();

    // Shell completion generation is completely independent, so perform it before
    // any config or subcommand operations.
    if let Ok(shell) = matches.value_of_t::<Shell>("completions") {
        generate(shell, &mut app, env!("CARGO_PKG_NAME"), &mut io::stdout());
        return Ok(());
    }

    #[allow(clippy::unwrap_used)]
    let config_dir = Path::new(matches.value_of_os("config-dir").unwrap());
    log::debug!("config dir: {:?}", config_dir);
    std::fs::create_dir_all(&config_dir)?;

    // There are two special cases that are not handled in `run`:
    //
    // * `kbs2` (no subcommand): Act as if a long --help message was requested and exit.
    // * `kbs2 init`: We're initializing a config instead of loading one.
    if matches.subcommand().is_none() {
        return app
            .clone()
            .print_long_help()
            .with_context(|| "failed to print help".to_string());
    } else if let Some(("init", matches)) = matches.subcommand() {
        return kbs2::command::init(matches, config_dir);
    }

    // Everything else (i.e., all other subcommands) go through here.
    let config = kbs2::config::load(&config_dir)?;
    match run(&matches, &config) {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Some(error_hook) = &config.error_hook {
                log::debug!("error-hook: {}", error_hook);
                config.call_hook(error_hook, &[&e.to_string()])?;
            }

            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app() {
        app().debug_assert();
    }
}
