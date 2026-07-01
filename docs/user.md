# User

Some machine setup belongs to the user account. The common examples are the
login shell and Linux group membership.

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

`dots` resolves the shell from `PATH`, so `"zsh"` becomes a full path such as
`/usr/bin/zsh`. If the current login shell already matches, the check has no
change.

On apply, `dots` runs:

```sh
chsh -s <shell-path>
```

Restart the login session for the shell change to take effect.

## Groups

Use `dots.user.groups(...)` for Linux groups:

```lua
dots.user.groups({ "docker" })
```

If the current user is not in the group, the check shows an add. On apply,
`dots` runs:

```sh
sudo usermod -aG docker "$USER"
```

Group membership usually requires logging out and back in before it affects new
sessions.
