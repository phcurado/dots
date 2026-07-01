# User

Some setup belongs to the user account itself. The common example is the login
shell.

```lua
dots.user.shell("zsh")
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

## Groups on Linux

Group management is Linux-only for now. On macOS, `dots.group.create(...)` and
`dots.user.add_to_groups(...)` report a conflict instead of trying to guess the
right `dscl` or Directory Services behavior.

Groups and group membership are separate.

Use `dots.group.create(...)` when the group itself should exist:

```lua
dots.group.create({ "media" })
```

On apply, missing groups are created with:

```sh
sudo groupadd media
```

Use `dots.user.add_to_groups(...)` when the current user should be a member of
existing groups:

```lua
dots.user.add_to_groups({ "docker", "wheel", "media" })
```

On apply, `dots` runs:

```sh
sudo usermod -aG docker "$USER"
```

If a group is used in `dots.user.add_to_groups(...)` but does not exist and is
not declared with `dots.group.create(...)`, `dots check` reports a conflict
instead of creating it by accident.

Groups declared with `dots.group.create(...)` are tracked when `dots check` or
`dots apply` sees that they exist. If that declaration is removed later,
`dots check` shows the group removal. Memberships declared with
`dots.user.add_to_groups(...)` are tracked the same way.

Group membership usually requires logging out and back in before it affects new
sessions.
