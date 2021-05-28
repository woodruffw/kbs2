# Changelog
All notable changes to `kbs2` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

All versions prior to 0.2.1 are untracked.

<!-- @next-header@ -->

## [Unreleased] - ReleaseDate

### Added

* CLI: `kbs2 dump` can now dump multiple records in one invocation
([#191](https://github.com/woodruffw/kbs2/pull/191))
* CLI: `kbs2 rm` can now remove multiple records in one invocation
([#195](https://github.com/woodruffw/kbs2/pull/195))

### Changed

* Agent: The agent's internal representation and protocol have been refactored.
Releases earlier than this one use an incompatible protocol; users should
run `kbs2 agent flush -q` after upgrading to kill their outdated agent
([#193](https://github.com/woodruffw/kbs2/pull/193))

### Fixed

* Contrib: `kbs2 choose-pass` no longer incorrectly nags the user when `choose`
is canceled.

## [0.3.0] - 2021-05-02

### Added

* CLI: `kbs2 rekey` enables users to rekey their entire secret store, re-encrypting
all records with a new secret key. `kbs2 rekey` also handles the chore work of
updating the user's config and related files for the new key.

### Changed

* Contrib: The `kbs2-dmenu-pass` and `kbs2-choose-pass` commands now understand the
`notify-username` (`bool`) setting, which allows them to send a desktop notification
for the copied record's username.
* Config, Contrib: External commands now use the `[commands.ext.<command>]` namespace
instead of `[commands.<command>]`.

## [0.2.6] - 2021-02-20

### Added

* Meta: The CHANGELOG and README are now semi-managed by `cargo release`
* Contrib: Added `kbs2-ssh-add`
* Control: Added `kbs2-gpg-add`
* Contrib: `kbs2-snip` can now print instead of running snippet with `-p`, `--print`
* CLI: Custom subcommands now receive `KBS2_MAJOR_VERSION`, `KBS2_MINOR_VERSION`, and
`KBS2_PATCH_VERSION` in their environments
* CLI: `kbs2 list` and `kbs2 dump` now use a more Unix-y format output

### Changed

* Backend: The encryption backend now uses a default work factor of `22`, up from `18`

## [0.2.5] - 2020-12-12

### Fixed

* Tests: Removed some overly conservative assertions with config directories

## [0.2.4] - 2020-12-10

### Fixed

* CLI: Fixed the functionality of `kbs2 init --insecure-not-wrapped`, broken
during an earlier refactor

## [0.2.3] - 2020-12-10

### Added

* CLI: `kbs2 init` now supports `-s`/`--store-dir` for configuring the record store at
config initialization time ([#123](https://github.com/woodruffw/kbs2/pull/118))

## [0.2.2] - 2020-12-06

### Added

* Config: `agent-autostart` now controls whether `kbs2 agent` is auto-spawned whenever a session is
requested ([#118](https://github.com/woodruffw/kbs2/pull/118))

### Changed

* Agent: Users no longer have to manually run `kbs2 agent`; most commands will now auto-start the
agent by default ([#118](https://github.com/woodruffw/kbs2/pull/118))

### Fixed

* Config: `wrapped` now always defaults to `true` ([#118](https://github.com/woodruffw/kbs2/pull/118))

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

<!-- @next-url@ -->
[Unreleased]: https://github.com/woodruffw/kbs2/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/woodruffw/kbs2/compare/v0.2.6...v0.3.0
[0.2.6]: https://github.com/woodruffw/kbs2/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/woodruffw/kbs2/releases/tag/v0.2.5
[0.2.4]: https://github.com/woodruffw/kbs2/releases/tag/v0.2.4
[0.2.3]: https://github.com/woodruffw/kbs2/releases/tag/v0.2.3
[0.2.2]: https://github.com/woodruffw/kbs2/releases/tag/v0.2.2
[0.2.1]: https://github.com/woodruffw/kbs2/releases/tag/v0.2.1
