`kbs2-choose-pass`
=================

`kbs2-choose-pass` is an external `kbs2` command that uses `choose` to
select a login and `kbs2 pass` to copy the selected login's password
to the clipboard.

`kbs2-choose-pass` only works on macOS.

## Setup

`kbs2-choose-pass` requires [`choose`](https://github.com/chipsenkbeil/choose), which is available
via Homebrew:

```bash
$ brew install choose-gui
```

## Usage

From the command line:

```bash
kbs2 choose-pass
```

Alternatively, if you have a Touch Bar, you can use `kbs2-choose-pass` as a "Quick Action".
An installable workflow is provided in this directory.
