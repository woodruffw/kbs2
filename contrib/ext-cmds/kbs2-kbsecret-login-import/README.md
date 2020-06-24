`kbs2-kbsecret-login-import`
==========================

`kbs2-kbsecret-login-import` is an external `kbs2` command that imports all login records
from a KBSecret session into the `kbs2` store.

## Setup

`kbsecret` is required.

## Usage

Import login records from the default session:

```bash
$ kbs2 kbsecret-login-import
```

Import login records from the "extra-logins" session:

```bash
$ kbs2 kbsecret-login-import extra-logins
```
