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

[`toml2json`](https://github.com/woodruffw/toml2json) and `jq` are optional
dependencies. See the configuration section for details.

## Configuration

`kbs2 choose-pass` reads the `commands.ext.choose-pass.notify-username` setting. If `true`,
a desktop notification is emitted containing the username of the record that
the user has selected (and is currently in the clipboard).

To read the configuration, `kbs2 choose-pass` requires both `toml2json` and `jq`.
If either is missing, the configuration will be silently ignored.

## Usage

From the command line:

```bash
kbs2 choose-pass
```

### "Quick Action" (Touch Bar)

If you have a Touch Bar, you can use `kbs2-choose-pass` as a "Quick Action".

An installable workflow is provided in this directory.

### Karabiner-Elements

Alternatively, if you use [Karabiner-Elements](https://github.com/pqrs-org/Karabiner-Elements),
you can use the binding provided [here](./kbs2-choose-pass.karabiner.json). By default, F12
(Function-12) is bound to `kbs2 choose-pass`.

```bash
cp kbs2-choose-pass.karabiner.json ~/.config/karabiner/assets/complex_modifications/kbs2.json
```

...and add "`kbs2-choose-pass`" in the "complex modifications" pane.
