`kbs2-gpg-add`
===========

`kbs2-gpg-add` is an external `kbs2` command that loads an GPG passphrase stored in `kbs2`
into your GPG agent via `gpg-preset-passphrase`.

## Setup

`kbs2-gpg-add` requires GnuPG2 and `jq`.

To use it, you'll need to configure your GPG agent to allow preset passphrases:

```
# ~/.gnupg/gpg-agent.conf or wherever
allow-preset-passphrase
```

Then, restart your GPG agent. Sending it `SIGHUP` won't work for this setting.

You'll also need to get your GPG keygrip:

```bash
gpg --list-keys --with-keygrip
```

Finally, load your GPG keygrip and passphrase into `kbs2`:

```bash
# set your username to your keygrip, and password to your passphrase.
kbs2 new gpg-passphrase-record
```

## Usage

`kbs2-gpg-add` loads the given record into your GPG agent, associating
the passphrase with the keygrip:

```bash
$ kbs2 gpg-add gpg-passphrase-record
```
