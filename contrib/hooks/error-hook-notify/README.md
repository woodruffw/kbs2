`error-hook-notify`
=================

`error-hook-notify` is a `kbs2` hook that displays a desktop notification
when a `kbs2` subcommand fail. Failures in external subcommands are also
reported.

`error-hook-notify` supports Linux and macOS.

## Setup

`error-hook-notify` requires `notify-send` on Linux. Most desktop
distributions should include it.

No special setup is required on macOS.

## Use

Configure `error-hook-notify` as the `error-hook` for `kbs2`:

```toml
error-hook = "~/.config/kbs2/hooks/error-hook-notify"
```
