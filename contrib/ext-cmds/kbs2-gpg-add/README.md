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

You'll also need to load your GPG passphrase (or passphrases) into `kbs2`:

```bash
kbs2 new gpg-passphrase
```

Finally, you'll need your key's GPG keygrip:

```
gpg --list-keys --with-keygrip
```

## Usage

`kbs2-gpg-add` loads the given record into your GPG agent, associated with the
given keygrip:

```bash
$ kbs2 gpg-add your-keygrip gpg-passphrase
```
