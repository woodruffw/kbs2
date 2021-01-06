`kbs2-ssh-add`
===========

`kbs2-ssh-add` is an external `kbs2 command` that loads an SSH key stored in `kbs2`
into your SSH agent via `ssh-add`.

## Setup

`kbs2-ssh-add` requires `ssh-add` (obviously) and `jq`.

To use it, load your SSH key of choice into `kbs2`:

```bash
# replace id_ed25519 with id_rsa or whatever your actual key is
kbs2-new -k unstructured your-ssh-key --terse < ~/.ssh/id_ed25519
```

## Usage

`kbs2-ssh-add` loads the given record into your SSH agent:

```bash
$ kbs2 ssh-add your-ssh-key
```

Note that you'll still be prompted for your SSH key's password if you choose to additionally
password-protect it.
