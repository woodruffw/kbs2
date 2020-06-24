`kbs2-snip`
===========

`kbs2-snip` is an external `kbs2 command` that uses
[`selecta`](https://github.com/garybernhardt/selecta) (or another fuzzy finder)
to find and execute a snippet of code stored as an unstructured record.

## Setup

By default, `kbs2-snip` requires `selecta`.

See the configuration options below for alternatives.

## Configuration

`kbs2-snip` reads the `commands.snip.matcher` setting in the configuration
file to determine which fuzzy finder to use.

For example:

```toml
[commands.snip]
matcher = "fzf"
```

...will cause `kbs2-snip` to use [`fzf`](https://github.com/junegunn/fzf).

## Usage

`kbs2-snip` searches for unstructured records whose contents begin with `snippet:`.

```bash
$ kbs2 new -k unstructured ls-tmp <<< "snippet:ls /tmp"
$ kbs2 snip
```
