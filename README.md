kbs2
====

[![Build Status](https://img.shields.io/github/workflow/status/woodruffw/kbs2/CI/master)](https://github.com/woodruffw/kbs2/actions?query=workflow%3ACI)

**Warning! `kbs2` is alpha-quality software! Using `kbs2` means accepting that your secrets may be lost or compromised at any time!**

`kbs2` is a command line utility for managing *secrets*.

`kbs2` uses the age Rust crate by default, although it can be
configured to use any [age](https://github.com/FiloSottile/age)-compatible CLI.

Quick links:

* [Installation](#installation)
* [Quick start guide](#quick-start-guide)
* [CLI documentation](#cli-documentation)
* [Configuration and customization](#configuration-and-customization)
* [Why another password manager?](#why-another-password-manager)
* [Technical details](#technical-details)
* [Hacking](#hacking)
* [History](#history)

## Installation

`kbs2` is most easily installed via `cargo`:

```bash
$ cargo install kbs2
```

After installation, `kbs2` is completely ready for use. See the
[Configuration](#configuration) section for some *optional* changes that you can
make, like switching out the use of the [age crate](https://docs.rs/age/)
for an `age`-compatible CLI.

## Quick start guide

Initialize a new `kbs2` configuration:

```bash
$ kbs2 init
```

`kbs2 init` will automatically discover an appropriate age CLI and generate a keypair with it.

Create a new login record:

```bash
$ kbs2 new login amazon
Username? jonf-bonzo
Password? (hidden)
```

List available records:

```bash
$ kbs2 list
amazon
facebook
```

Pull the password from a record:

```bash
$ kbs2 pass -c amazon
# alternatively, pipeline it
$ kbs2 pass facebook | pbcopy
```

Remove a record:

```bash
$ kbs2 rm facebook
```

`kbs2`'s subcommands are substantially more featured than the above examples demonstrate;
run each with `--help` to see a full set of supported options.

## CLI documentation

None yet. Watch this space.

## Configuration and customization

### Configuration

`kbs2` stores its configuration in `<config dir>/kbs2/kbs2.conf`, where `<config dir>` is determined
by your host system. On Linux, for example, it's `~/.config`.

`kbs2.conf` is TOML-formatted, and might look something like this after a clean start with `kbs2 init`:

```toml
age-backend = "RageLib"
public-key = "age1elujxyndwy0n9j2e2elmk9ns8vtltg69q620dr0sz4nu5fgj95xsl2peea"
keyfile = "/home/william/.config/kbs2/key"
store = "/home/william/.local/share/kbs2"

[commands.pass]
clipboard-duration = 10
clear-after = true
x11-clipboard = "Clipboard"
```

#### `age-backend` (default: `"RageLib"`)

The `age-backend` setting tells `kbs2` how to operate on age-formatted keypairs and encrypted
records. The supported options are `"RageLib"`, `"AgeCLI`", and `"RageCLI"`:

* `"RageLib"`: Use the [age crate](https://docs.rs/age/) for all age operations. This is the default
setting, and offers the best performance.

* `"AgeCLI"`: Use the `age` and `age-keygen` binaries for all all age operations. This setting
requires that `age` and `age-keygen` are already installed; see the
[age README](https://github.com/FiloSottile/age#installation) for instructions.

* `"RageCLI"`: Use the `rage` and `rage-keygen` binaries for all age operations. This setting
requires that `rage` and `rage-keygen` are already installed; see the
[rage README](https://github.com/str4d/rage#installation) for instructions.

#### `public-key` (default: generated by `kbs2 init`)

The `public-key` setting records the public half of the age keypair used by `kbs2`.

`kbs2 init` pre-populates this setting; users should **not** modify it **unless** also modifying
the `keyfile` setting (e.g., to point to a pre-existing age keypair).

#### `keyfile` (default: generated by `kbs2 init`)

The `keyfile` setting records the path to the private half of the age keypair used by `kbs2`.

`kbs2 init` pre-populates this setting; users should **not** modify it **unless** also modifying
the `public-key` setting (e.g., to point to a pre-existing age keypair).

#### `store` (default: `<user data directory>/kbs2`)

The `store` setting records the path to the secret store, i.e. where records are kept.

Users may modify this setting to store their records in custom directory.

#### `pre-hook` (default: `None`)

The `pre-hook` setting can be used to run a command before (almost) every `kbs2` invocation.

Read the [Hooks](#hooks) documentation for more details.

#### `post-hook` (default: `None`)

The `post-hook` setting can be used to run a command after (almost) every `kbs2` invocation.

Read the [Hooks](#hooks) documentation for more details.

#### `reentrant-hooks` (default: `false`)

The `reentrant-hooks` setting controls whether hooks are run multiple times when a hook itself
runs `kbs2`. By default, hooks are run only for the initial `kbs2` invocation.

Read the [Reentrancy section](#reentrancy) of the [Hooks](#hooks) documentation for more details.

#### `commands.new.pre-hook` (default: `None`)

The `commands.new.pre-hook` setting is like the global `pre-hook` setting, except that it runs
immediately before record creation during `kbs2 new` (and **only** `kbs2 new`).

#### `commands.new.post-hook` (default: `None`)

The `commands.new.post-hook` setting is like the global `post-hook` setting, except that it runs
immediately after record creation during `kbs2 new` (and **only** `kbs2 new`).

The `commands.new.post-hook` setting passes a single argument to its hook, which is the label
of the record that was just created. For example, the following:

```toml
[commands.new]
post-hook = "~/.config/kbs2/hooks/post-new.sh"
```

```bash
# ~/.config/kbs2/hooks/post-new.sh

>&2 echo "[+] created ${1}"
```

would produce:

```bash
$ kbs2 new login foo
Username: bar
Password: [hidden]
[+] created foo
```

#### `commands.pass.clipboard-duration` (default: `10`)

The `commands.pass.clipboard-duration` setting determines the duration, in seconds, for persisting
a password stored in the clipboard via `kbs2 pass -c`.

#### `commands.pass.clear-after` (default: `true`)

The `commands.pass.clear-after` setting determines whether or not the clipboard is cleared at
all after `kbs2 pass -c`.

Setting this to `false` overrides any duration configured in `commands.pass.clipboard-duration`.

#### `commands.pass.x11-clipboard` (default: `"Clipboard"`)

*This setting has no functionality yet; see [#3](https://github.com/woodruffw/kbs2/issues/3)*.

The `commands.pass.x11-clipboard` setting determines which clipboard is used on X11.

Valid options are `"Clipboard"` and `"Primary"`.

#### `commands.edit.editor` (default: `None`)

The `commands.edit.editor` setting controls which editor is used when opening a file with
`kbs2 edit`. The `$EDITOR` environment variable takes precedence over this setting.

This setting is allowed to contain flags. For example, the following would be split correctly:

```toml
[commands.edit]
editor = "subl -w"
```

### Customization

`kbs2` offers several avenues for customization.

#### Custom commands

`kbs2` supports `git`-style subcommands, allowing you to easily write your own.

For example, running the following:

```
$ kbs2 frobulate --xyz
```

will cause `kbs2` to run `kbs2-frobulate --xyz`. Custom commands are allowed to read from and
write to the config file under the `[commands.<name>]` hierarchy.

The [kbs2-ext-cmds](https://github.com/woodruffw/kbs2-ext-cmds) repository contains several useful
external commands.

#### Hooks

`kbs2` exposes hook-points during the lifecycle of an invocation, allowing users to
inject additional functionality or perform their own bookkeeping.

##### The hook API

All hooks, whether pre- or post-, have the following behavior:

* Hooks **do not** inherit `stdin` or `stdout` from the parent `kbs2` process
* Hooks **do** inherit `stderr` from the parent process, and *may* use it to print anything
they please
* Hooks **always** run from the `store` directory
* Hooks are run with `KBS2_HOOK=1` in their environment
* An error exit from a hook (or failure to execute) causes the entire `kbs2` command to fail

Hooks *may* introduce additional behavior, so long as it does not conflict with the above.
Any additional hook behavior is documented under that hook's configuration setting.

##### Reentrancy

`kbs2`'s hooks are non-reentrant by default.

To understand what that means, imagine the following hook setup:

```toml
pre-hook = "~/.config/kbs2/hooks/pre.sh"
```

```bash
# ~/.config/kbs2/hooks/pre.sh

kbs2 some-other-command
```

and then:

```bash
$ kbs2 list
```

In this setting, most users would expect `pre.sh` to be run exactly once: on `kbs2 list`.

However, naively, it *ought* to execute twice: once for `kbs2 list`, and again for
`kbs2 some-other-command`. In other words, naively, hooks would *reenter* themselves whenever
they use `kbs2` internally.

Most users find this confusing and would consider it an impediment to hook writing, so `kbs2`
does **not** do this by default. However, **should** you wish for reentrant hooks, you have two
options:

* You can set `reentrant-hooks` to `true` in the configuration. This will make *all* hooks
reentrant &mdash; it's all or nothing, intentionally.
* You can `unset` or otherwise delete the `KBS2_HOOK` environment variable in your hook
before running `kbs2` internally. This allows you to control which hooks cause reentrancy.
**Beware**: `KBS2_HOOK` is an implementation detail! Unset it at your own risk!

## Why another password manager?

No good reason. See the [history section](#history).

## Technical details

## Hacking

## History

TL;DR: `kbs2` is short for "[KBSecret](https://github.com/kbsecret/kbsecret) 2".

In 2017, I wrote KBSecret as a general purpose secret manager for the Keybase ecosystem.

KBSecret was written in Ruby and piggybacked off of Keybase + KBFS for encryption, storage,
and synchronization. It was also *extremely* flexible, allowing user-defined record types, secret
sharing between users and teams, and a variety of convenient and well-behaved CLI tools for
integration into my development ecosystem.

Unfortunately, KBSecret was also *extremely* slow: it was written in
[obnoxiously metaprogrammed Ruby](https://github.com/kbsecret/kbsecret/blob/20ac2bf/lib/kbsecret/config.rb#L175),
relied heavily on re-entrant CLIs, and was further capped by the latency and raw performance of KBFS
itself.

Having a slow secret manager was fine for my purposes, but I
[no longer trust](https://keybase.io/blog/keybase-joins-zoom) that Keybase (and KBFS) will continue
to receive the work they require. I also no longer have the time to maintain KBSecret's (slowly)
deteriorating codebase.

`kbs2` is my attempt to reproduce the best parts of KBSecret in a faster language. Apart from the
name and some high-level design decisions, it shares nothing in common with the original KBSecret.
It's only named `kbs2` because I'm used to typing "kbs" in my terminal.
