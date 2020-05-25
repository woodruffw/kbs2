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
* [Configuration](#configuration)
  * [Generators](#generators)
* [Customization](#customization)
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

`kbs2 init` will automatically generate a configuration file and keypair, prompting you for
a "master" password.

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

### `kbs2 init`

#### Usage

```
initialize kbs2 with a new config and keypair

USAGE:
    kbs2 init [FLAGS]

FLAGS:
    -f, --force                   overwrite the config and keyfile, if already present
    -h, --help                    Prints help information
        --insecure-not-wrapped    don't wrap the keypair with a master password
```

#### Examples

Create a new config and keypair, prompting the user for a master password:

```bash
$ kbs2 init
```

Create a new config and keypair **without** a master password:

```bash
$ kbs2 init --insecure-not-wrapped
```

### `kbs2 unlock`

#### Usage

```
unwrap the private key for use

USAGE:
    kbs2 unlock

FLAGS:
    -h, --help    Prints help information
```

#### Examples

Unwrap the private key, allowing future commands to run without the master password:

```bash
$ kbs2 unlock
```

### `kbs2 lock`

#### Usage

```
remove the unwrapped key, if any, from shared memory

USAGE:
    kbs2 lock

FLAGS:
    -h, --help    Prints help information
```

#### Examples

Remove the unwrapped private key from shared memory, requiring future commands to prompt for
the master password:

```bash
$ kbs2 lock
```

### `kbs2 new`

#### Usage

```
create a new record

USAGE:
    kbs2 new [FLAGS] [OPTIONS] <kind> <label>

ARGS:
    <kind>     the kind of record to create [possible values: login, environment, unstructured]
    <label>    the record's label

FLAGS:
    -f, --force       overwrite, if already present
    -g, --generate    generate sensitive fields instead of prompting for them
    -h, --help        Prints help information
    -t, --terse       read fields in a terse format, even when connected to a tty

OPTIONS:
    -G, --generator <generator>    use the given generator to generate sensitive fields [default: default]
```

#### Examples

Create a new `login` record named `foobar`:

```bash
$ kbs2 new login foobar
Username: hasdrubal
Password: [hidden]
```

Create a new `environment` record named `twitter-api`, overwriting it if it already exists:

```bash
$ kbs2 new -f environment twitter-api
Variable: TWITTER_API
Value: [hidden]
```

Create a new `login` record named `pets.com`, generating the password with the default generator:

```bash
$ kbs2 new -g login pets.com
Username: hasdrubal
```

Create a new `login` record named `email`, getting the fields in a terse format:

```bash
$ kbs2 new -t login email < <(echo -e "bill@microsoft.com\x01hunter2")
```

### `kbs2 list`

#### Usage

```
list records

USAGE:
    kbs2 list [FLAGS] [OPTIONS]

FLAGS:
    -d, --details    print (non-field) details for each record
    -h, --help       Prints help information

OPTIONS:
    -k, --kind <kind>    list only records of this kind [possible values: login, environment, unstructured]
```

#### Examples

List all records, one per line:

```bash
$ kbs2 list
foobar
twitter-api
pets.com
email
```

List (non-sensitive) details for each record:

```bash
$ kbs2 list -d
foobar
  Kind: login
  Timestamp: 1590277900
twitter-api
  Kind: environment
  Timestamp: 1590277907
pets.com
  Kind: login
  Timestamp: 1590277920
email
  Kind: login
  Timestamp: 1590277953
```

List only environment records:

```bash
$ kbs2 list -k environment
twitter-api
```

### `kbs2 rm`

#### Usage

```
remove a record

USAGE:
    kbs2 rm <label>

ARGS:
    <label>    the record's label

FLAGS:
    -h, --help    Prints help information
```

#### Examples

Remove the `foobar` record:

```bash
$ kbs2 rm foobar
```

### `kbs2 dump`

#### Usage

```
dump a record

USAGE:
    kbs2 dump [FLAGS] <label>

ARGS:
    <label>    the record's label

FLAGS:
    -h, --help    Prints help information
    -j, --json    dump in JSON format
```

#### Examples

Dump the `twitter-api` record:

```bash
$ kbs2 dump twitter-api
Label: twitter-api
  Kind: environment
  Variable: TWITTER_API
  Value: 92h2890fn83fb2378fbf283bf73fbxkfnso90
```

Dump the `pets.com` record in JSON format:

```bash
$ kbs2 dump -j pets.com | json_pp
{
   "timestamp" : 1590363392,
   "label" : "pets.com",
   "body" : {
      "fields" : {
         "username" : "hasdrubal",
         "password" : "hunter2"
      },
      "kind" : "Login"
   }
}

```

### `kbs2 pass`

#### Usage

```
get the password in a login record

USAGE:
    kbs2 pass [FLAGS] <label>

ARGS:
    <label>    the record's label

FLAGS:
    -c, --clipboard    copy the password to the clipboard
    -h, --help         Prints help information
```

#### Examples

Get the password for the `pets.com` record:

```bash
$ kbs2 pass pets.com
hunter2
```

Copy the password for the `pets.com` record into the clipboard:

```bash
$ kbs2 pass -c pets.com
```

### `kbs2 env`

#### Usage

```
get an environment record

USAGE:
    kbs2 env [FLAGS] <label>

ARGS:
    <label>    the record's label

FLAGS:
    -h, --help          Prints help information
    -n, --no-export     print only VAR=val without `export`
    -v, --value-only    print only the environment variable value, not the variable name
```

#### Examples

Get an environment record in `export`-able form:

```bash
$ kbs2 env twitter-api
export TWITTER_API=92h2890fn83fb2378fbf283bf73fbxkfnso90
```

Get just the value in an environment record:

```bash
$ kbs2 env -v twitter-api
92h2890fn83fb2378fbf283bf73fbxkfnso90
```

### `kbs2 edit`

#### Usage

```
modify a record with a text editor

USAGE:
    kbs2 edit [FLAGS] <label>

ARGS:
    <label>    the record's label

FLAGS:
    -h, --help                  Prints help information
    -p, --preserve-timestamp    don't update the record's timestamp
```

#### Examples

Open the `email` record for editing:

```bash
$ kbs2 edit email
```

Open the `email` record for editing with a custom `$EDITOR`:

```bash
$ EDITOR=vim kbs2 edit email
```

## Configuration

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

### `age-backend` (default: `"RageLib"`)

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

### `public-key` (default: generated by `kbs2 init`)

The `public-key` setting records the public half of the age keypair used by `kbs2`.

`kbs2 init` pre-populates this setting; users should **not** modify it **unless** also modifying
the `keyfile` setting (e.g., to point to a pre-existing age keypair).

### `keyfile` (default: generated by `kbs2 init`)

The `keyfile` setting records the path to the private half of the age keypair used by `kbs2`.

`kbs2 init` pre-populates this setting; users should **not** modify it **unless** also modifying
the `public-key` setting (e.g., to point to a pre-existing age keypair).

### `wrapped` (default: `true`)

The `wrapped` settings records whether `keyfile` is a "wrapped" private key, i.e. whether
the private key itself is encrypted with a master password.

By default, `kbs2 init` asks the user for a master password and creates a wrapped key.
See the [`kbs2 init`](#kbs2-init) documentation for more information.

### `store` (default: `<user data directory>/kbs2`)

The `store` setting records the path to the secret store, i.e. where records are kept.

Users may modify this setting to store their records in custom directory.

### `pre-hook` (default: `None`)

The `pre-hook` setting can be used to run a command before (almost) every `kbs2` invocation.

Read the [Hooks](#hooks) documentation for more details.

### `post-hook` (default: `None`)

The `post-hook` setting can be used to run a command after (almost) every `kbs2` invocation.

There are currently four cases when the configured `post-hook` will *not* run:

* `kbs2` (i.e., no subcommand)
* `kbs2 init`
* `kbs2 unlock`
* `kbs2 lock`

All other subcommands, including custom subcommands, will cause the configured `post-hook` to run.

Read the [Hooks](#hooks) documentation for more details.

### `reentrant-hooks` (default: `false`)

The `reentrant-hooks` setting controls whether hooks are run multiple times when a hook itself
runs `kbs2`. By default, hooks are run only for the initial `kbs2` invocation.

Read the [Reentrancy section](#reentrancy) of the [Hooks](#hooks) documentation for more details.

### `commands.new.pre-hook` (default: `None`)

The `commands.new.pre-hook` setting is like the global `pre-hook` setting, except that it runs
immediately before record creation during `kbs2 new` (and **only** `kbs2 new`).

### `commands.new.post-hook` (default: `None`)

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

### `commands.pass.clipboard-duration` (default: `10`)

The `commands.pass.clipboard-duration` setting determines the duration, in seconds, for persisting
a password stored in the clipboard via `kbs2 pass -c`.

### `commands.pass.clear-after` (default: `true`)

The `commands.pass.clear-after` setting determines whether or not the clipboard is cleared at
all after `kbs2 pass -c`.

Setting this to `false` overrides any duration configured in `commands.pass.clipboard-duration`.

### `commands.pass.x11-clipboard` (default: `"Clipboard"`)

*This setting has no functionality yet; see [#3](https://github.com/woodruffw/kbs2/issues/3)*.

The `commands.pass.x11-clipboard` setting determines which clipboard is used on X11.

Valid options are `"Clipboard"` and `"Primary"`.

### `commands.pass.pre-hook` (default: `None`)

The `command.pass.pre-hook` setting is like the global `pre-hook` setting, except that it runs
immediately before record access during `kbs2 pass` (and **only** `kbs2 pass`).

### `command.pass.post-hook` (default: `None`)

The `command.pass.post-hook` setting is like the global `post-hook` setting, except that it runs
immediately after record access during `kbs2 pass` (and **only** `kbs2 pass`).

### `command.pass.clear-hook` (default: `None`)

The `command.pass.clear-hook` is like the other `command.pass` hooks, except that it only runs
after the password has been cleared from the clipboard.

### `commands.edit.editor` (default: `None`)

The `commands.edit.editor` setting controls which editor is used when opening a file with
`kbs2 edit`. The `$EDITOR` environment variable takes precedence over this setting.

This setting is allowed to contain flags. For example, the following would be split correctly:

```toml
[commands.edit]
editor = "subl -w"
```

### `commands.rm.post-hook` (default: `None`)

The `command.rm.post-hook` setting is like the global `post-hook` setting, except that it runs
immediately after record removal during `kbs2 rm` (and **only** `kbs2 rm`).

### Generators

`kbs2` supports *generators* for producing sensitive values, allowing users to automatically
generate passwords and environment variables.

Generators come in two flavors: "command" generators and "internal" generators. Both are
configured as entries in `[[generators]]`.

The following configures two generators: a "command" generator named "pwgen" that executes
`pwgen` to get a new secret, and an "internal" generator named "hexonly" that generates
a secret from the configured alphabet and length.

```toml
[[generators]]
name = "pwgen"
command = "pwgen 16 1"

[[generators]]
name = "hexonly"
alphabet = "0123456789abcdef"
length = 16
```

These generators can be used with `kbs2 new`:

```bash
# Notice: the user is not prompted for a password
$ kbs2 new -gG hexonly login pets.com
Username: catlover2000
```

## Customization

Beyond the configuration above, `kbs2` offers several avenues for customization.

### Custom commands

`kbs2` supports `git`-style subcommands, allowing you to easily write your own.

For example, running the following:

```
$ kbs2 frobulate --xyz
```

will cause `kbs2` to run `kbs2-frobulate --xyz`. Custom commands are allowed to read from and
write to the config file under the `[commands.<name>]` hierarchy.

The [kbs2-ext-cmds](https://github.com/woodruffw/kbs2-ext-cmds) repository contains several useful
external commands.

### Hooks

`kbs2` exposes hook-points during the lifecycle of an invocation, allowing users to
inject additional functionality or perform their own bookkeeping.

#### The hook API

All hooks, whether pre- or post-, have the following behavior:

* Hooks **do not** inherit `stdin` or `stdout` from the parent `kbs2` process
* Hooks **do** inherit `stderr` from the parent process, and *may* use it to print anything
they please
* Hooks **always** run from the `store` directory
* Hooks are run with `KBS2_HOOK=1` in their environment
* An error exit from a hook (or failure to execute) causes the entire `kbs2` command to fail

Hooks *may* introduce additional behavior, so long as it does not conflict with the above.
Any additional hook behavior is documented under that hook's configuration setting.

#### Reentrancy

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

### Threat model

`kbs2`'s threat model is similar to that of most password and secret managers. In particular:

* `kbs2` does *not* attempt to defend against the `root` user *or* arbitrary code executed by the
current user.
* `kbs2` tries to avoid operations that would result in secret material (i.e. the private key
and the decrypted contents of records) being saved or cached on disk, but does *not* attempt to
present the consumers of secret material from doing so.
* `kbs2`, by default, attempts to prevent offline private key extraction by encrypting the private
key at rest with a master password. `kbs2` does *not* attempt to prevent the user from mishandling
their master password.

### Cryptography

`kbs2` does **not** implement any cryptography on its own &mdash; it uses *only* the cryptographic
primitives supplied by an [age](https://github.com/FiloSottile/age) implementation.

The particulars of `kbs2`'s cryptographic usage are as follows:

* Every `kbs2` configuration file specifies a symmetric keypair. The public key is
stored in the `public-key` configuration setting, while the private key is stored in the file
referenced by the `keyfile` setting.
* By default, `kbs2` "wraps" (i.e. encrypts) the private key with a master password. This makes
offline key extraction attacks more difficult (although not impossible) and makes the consequences
of wrapped private key disclosure less severe. Users *may* choose to use a non-wrapped key by
passing `--insecure-not-wrapped` to `kbs2 init`.

### Key unwrapping and persistence

As mentioned under [Threat Model](#threat-model) and [Cryptography](#cryptography), `kbs2` uses
a wrapped private key by default.

Without any persistence, wrapped key usage would be tedious: the user would have to re-enter
their master password on each `kbs2` action, defeating the point of having a secret manager.

To avoid this, `kbs2` establishes persistence of the unwrapped key with a POSIX shared memory
object (specifically, an object named `/__kbs2_unwrapped_key`). This is done after first use
*or* explicitly with `kbs2 unlock`. The unwrapped key can be de-persisted either by rebooting
the machine *or* by running `kbs2 lock`.

Unlike like `ssh-agent` and `gpg-agent`, `kbs2`'s shared memory object is *not* tied to a user's
session. This means that logging out and logging back in does *not* require the user to re-enter
their master password *unless* they have otherwise configured their system to run `kbs2 lock`
before the end of their session.

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
