`kbs2-kbsecret-env-import`
==========================

`kbs2-kbsecret-env-import` is an external `kbs2` command that imports all environment records
from a KBSecret session into the `kbs2` store.

## Setup

`kbsecret` is required.

## Usage

Import environment records from the default session:

```bash
$ kbs2 kbsecret-env-import
```

Import environment records from the "api-keys" session:

```bash
$ kbs2 kbsecret-env-import api-keys
```
