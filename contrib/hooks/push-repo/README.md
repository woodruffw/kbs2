`push-repo`
===========

`push-repo` is a `kbs2` hook that treats the `kbs2` store as a Git
repository, committing and pushing any changes made to it whenever
the hook is run.

## Setup

To use `push-repo`, initialize a Git repository in your `kbs2` store:

```bash
$ git init
$ git remote add origin https://your-git-remote-here.git
```

## Use

`push-repo` should be configured as the `post-hook` on any command(s) that
you regularly modify the store's state with.

For example:

```toml
[commands.new]
post-hook = "~/.config/kbs2/hooks/push-repo"

[commands.rm]
post-hook = "~/.config/kbs2/hooks/push-repo"

[commands.edit]
post-hook = "~/.config/kbs2/hooks/push-repo"
```

Alternatively, you can set `push-repo` as the global post hook:

```toml
post-hook = "~/.config/kbs2/hooks/push-repo"
```

Be aware, however, that doing this will make many `kbs2` actions slower.
