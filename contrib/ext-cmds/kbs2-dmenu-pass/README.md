`kbs2-dmenu-pass`
=================

`kbs2-dmenu-pass` is an external `kbs2` command that uses `dmenu` to
select a login and `kbs2 pass` to copy the selected login's password
to the clipboard.

`kbs2-dmenu-pass` only works on systems running X11.

## Setup

`kbs2-dmenu-pass` requires [`dmenu`](https://tools.suckless.org/dmenu/).
Your package manager should supply it.

[`toml2json`](https://github.com/woodruffw/toml2json) and `jq` are optional
dependencies. See the configuration section for details.

## Configuration

`kbs2 dmenu-pass` reads the `commands.ext.dmenu-pass.notify-username` setting. If `true`,
a desktop notification is emitted containing the username of the record that
the user has selected (and is currently in the clipboard).

To read the configuration, `kbs2 dmenu-pass` requires both `toml2json` and `jq`.
If either is missing, the configuration will be silently ignored.

## Usage

```bash
kbs2 dmenu-pass
```
