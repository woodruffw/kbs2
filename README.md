kbs2
====

**Warning! `kbs2` is alpha-quality software! You should probably use another secret manager!**

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

## Configuration and customization

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
