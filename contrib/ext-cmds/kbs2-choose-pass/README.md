`kbs2-choose-pass`
=================

`kbs2-choose-pass` is an external `kbs2` command that uses `choose` to
select a login and `kbs2 pass` to copy the selected login's password
to the clipboard.

`kbs2-choose-pass` only works macOS

## Setup

`kbs2-choose-pass` requires [`choose`](https://github.com/chipsenkbeil/choose), which is available
via Homebrew:

```bash
$ brew install choose-gui
```

## Usage

```bash
kbs2 choose-pass
```
