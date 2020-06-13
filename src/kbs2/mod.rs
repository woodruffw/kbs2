/// Structures and routines for interacting with age backends.
pub mod backend;

/// Routines for the various `kbs2` subcommands.
pub mod command;

/// Structures and routines for `kbs2`'s configuration.
pub mod config;

/// Structures and routines for secret generators.
pub mod generator;

/// Routines for handling user input.
pub mod input;

/// Structures and routines for creating and managing individual `kbs2` records.
pub mod record;

/// Structures and routines for creating and managing an active `kbs2` session.
pub mod session;

/// Reusable utility code for `kbs2`.
pub mod util;
