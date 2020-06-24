`pass-clear-notify`
=================

`pass-clear-notify` is a `kbs2` hook that displays a desktop notification
after the clipboard has been cleared by `kbs2 pass`.

`pass-clear-notify` supports Linux and macOS.

## Setup

`pass-clear-notify` requires `notify-send` on Linux. Most desktop
distributions should include it.

No special setup is required on macOS.

## Use

Configure `pass-clear-notify` as the `clear-hook` for `kbs2 pass`:

```toml
[commands.pass]
clear-hook = "~/.config/kbs2/hooks/pass-clear-notify"
```
