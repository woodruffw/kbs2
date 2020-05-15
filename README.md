kbs2
====

[![Build Status](https://img.shields.io/github/workflow/status/woodruffw/kbs2/CI/master)](https://github.com/woodruffw/kbs2/actions?query=workflow%3ACI)

**Warning! `kbs2` is alpha-quality software! Using `kbs2` means accepting that your secrets may be lost or compromised at any time!**

`kbs2` is a command line utility for managing *secrets*.

It uses [age](https://github.com/FiloSottile/age) (or an age-compatible CLI, like
[rage](https://github.com/str4d/rage)) for encryption and decryption.

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

To actually *use* `kbs2`, you'll need to have an age-compatible CLI installed.

You can install `rage` via `cargo` as well:

```bash
$ cargo install rage
```

Alternatively, you can install `age`. The `age` README documents
[several installation methods](https://github.com/FiloSottile/age#installation).

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
debug = false
age-backend = "rage"
age-keygen-backend = "rage-keygen"
public-key = "age1elujxyndwy0n9j2e2elmk9ns8vtltg69q620dr0sz4nu5fgj95xsl2peea"
keyfile = "/home/william/.config/kbs2/key"
store = "/home/william/.local/share/kbs2"

[commands.pass]
clipboard-duration = 10
clear-after = true
x11-clipboard = "Clipboard"
```

Documentation of each configuration field to come.

### Customization

`kbs2` supports `git`-style subcommands, allowing you to easily write your own.

For example, running the following:

```
$ kbs2 frobulate --xyz
```

will cause `kbs2` to run `kbs2-frobulate --xyz`. Custom commands are allowed to read from and
write to the config file under the `[commands.<name>]` hierarchy.

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
