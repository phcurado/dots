# User

Some setup belongs to the user account itself. For example, you may want `zsh`
as the login shell, or you may need to be in the `docker` group after installing
Docker.

Declare those settings with `dots.user`:

```lua
dots.user.shell("zsh")
dots.user.groups({ "docker" })
```

These settings apply to the user running `dots`.

## Shell

Use `dots.user.shell(...)` to set the login shell:

```lua
dots.user.shell("zsh")
```

`dots` resolves the shell with `PATH`, so `"zsh"` becomes something like
`/usr/bin/zsh`. If the current login shell already matches, the plan has no
change.

On apply, `dots` runs:

```sh
chsh -s <shell-path>
```

You may need to log out and back in before the new shell is used by terminals.

## Groups

Use `dots.user.groups(...)` for Linux groups:

```lua
dots.user.groups({ "docker" })
```

If the current user is not in the group, the plan shows an add. On apply,
`dots` runs:

```sh
sudo usermod -aG docker "$USER"
```

Group membership usually requires logging out and back in before it affects new
sessions.
