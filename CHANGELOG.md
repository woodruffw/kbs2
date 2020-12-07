# Changelog
All notable changes to `kbs2` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

All versions prior to 0.2.1 are untracked.

## Unreleased

### Added

* Config: `agent-autostart` now controls whether `kbs2 agent` is auto-spawned whenever a session is
requested ([#118](https://github.com/woodruffw/kbs2/pull/118))

### Changed

* Agent: Users no longer have to manually run `kbs2 agent`; most commands will now auto-start the
agent by default ([#118](https://github.com/woodruffw/kbs2/pull/118))

### Removed

### Fixed

* `wrapped` now always defaults to `true` ([#118](https://github.com/woodruffw/kbs2/pull/118))

## [0.2.1] - 2020-12-05

### Added

* Packaging: AUR is now supported. ([#89](https://github.com/woodruffw/kbs2/pull/89))
* CLI: `kbs2 agent` (and subcommands) now provide key persistence, replacing the original POSIX SHM
implementation ([#103](https://github.com/woodruffw/kbs2/pull/103))
* CLI: `kbs2 rewrap` enables users to change the master password on their wrapped key(s)
([#107](https://github.com/woodruffw/kbs2/pull/107))
* Config: Users can now specify a custom Pinentry binary for prompts via the `pinentry` field
([#108](https://github.com/woodruffw/kbs2/pull/108))
* Config, Hooks: Support for an `error-hook` was added
([#117](https://github.com/woodruffw/kbs2/pull/117))

### Changed

* External commands: external commands run via `kbs2 {EXTERNAL}` that exit with an error now
cause `kbs2` to exit with 1, instead of 2.

### Removed

* CLI: `kbs2 lock` and `kbs2 unlock` were removed entirely as part of the `kbs2 agent` refactor.

[0.2.1]: https://github.com/woodruffw/kbs2/releases/tag/v0.2.1
