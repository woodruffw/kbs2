`kbs2-git-ssh-signing`
======================

`kbs2-git-ssh-signing` is an external `kbs2` command that:

1. Loads an SSH key stored in `kbs2` into your SSH agent via `ssh-add`.
2. Emits the public component of the private key in a format
   that `git` understands, so that the output of this command
   can be used by `git`'s SSH signing support.

## Setup

`kbs2-git-ssh-signing` requires `ssh-add` (obviously) and `jq`.

To use it, load your SSH key of choice into `kbs2`:

```bash
# replace id_ed25519 with id_rsa or whatever your actual key is
kbs2-new -k unstructured your-ssh-key --terse < ~/.ssh/id_ed25519
```

## Usage

`kbs2-git-ssh-signing` loads the given record into your SSH agent, and
prints it in the `key::` format that `git` expects for SSH signing keys:

```bash
$ kbs2 git-ssh-signing your-ssh-signing-key
key::ssh-ed25519 YOUR-PUBKEY-HERE YOUR@IDENTITY.HERE
```

This can done on-demand via the following `~/.gitconfig` settings:

```ini
[gpg "ssh"]
    defaultKeyCommand = kbs2 git-ssh-signing your-ssh-signing-key
```

Note that you'll still be prompted for your SSH key's password if you choose to additionally
password-protect it.

## Resources

* [`git`'s documentation for `gpg.ssh.defaultKeyCommand`](https://git-scm.com/docs/git-config#Documentation/git-config.txt-gpgsshdefaultKeyCommand)
