# Changelog
All notable changes to `kbs2` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

All versions prior to 0.2.1 are untracked.

<!-- @next-header@ -->

## [Unreleased] - ReleaseDate

### Added

* Meta: The CHANGELOG and README are now semi-managed by `cargo release`
* Contrib: Added `kbs2-ssh-add`
* Control: Added `kbs2-gpg-add`
* CLI: Custom subcommands now receive `KBS2_MAJOR_VERSION`, `KBS2_MINOR_VERSION`, and
`KBS2_PATCH_VERSION` in their environments

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
[Unreleased]: https://github.com/woodruffw/kbs2/compare/v0.2.5...HEAD
[0.2.5]: https://github.com/woodruffw/kbs2/releases/tag/v0.2.5
[0.2.4]: https://github.com/woodruffw/kbs2/releases/tag/v0.2.4
[0.2.3]: https://github.com/woodruffw/kbs2/releases/tag/v0.2.3
[0.2.2]: https://github.com/woodruffw/kbs2/releases/tag/v0.2.2
[0.2.1]: https://github.com/woodruffw/kbs2/releases/tag/v0.2.1
