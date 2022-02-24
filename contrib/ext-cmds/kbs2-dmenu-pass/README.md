`kbs2-dmenu-pass`
=================

`kbs2-dmenu-pass` is an external `kbs2` command that uses `dmenu` to
select a login and `kbs2 pass` to copy the selected login's password
to the clipboard.

`kbs2-dmenu-pass` only works on systems running X11.

## Setup

`kbs2-dmenu-pass` requires [`dmenu`](https://tools.suckless.org/dmenu/) by
default. Your package manager should supply it.

[`jq`](https://stedolan.github.io/jq/) is required for config handling.
Your package manager should supply it.

## Configuration

### `commands.ext.dmenu-pass.notify-username` (boolean)

If `true`, a desktop notification is emitted containing the username of the
record that the user has selected (and is currently in the clipboard).

### `commands.ext.dmenu-pass.chooser` (string)

If set, `kbs2-dmenu-pass` will execute this string as a `dmenu`-compatible chooser.

For example, to use [`rofi`](https://github.com/davatorium/rofi):

```toml
[commands.ext.dmenu-pass]
chooser = "rofi -dmenu -p kbs2"
```

## Usage

```bash
kbs2 dmenu-pass
```
