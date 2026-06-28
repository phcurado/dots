# dots documentation

## Workflow

Run from the dotfiles repository:

```sh
dots plan
dots apply
```

`plan` refreshes local state and shows the diff. `apply` performs the pending
changes and updates state.

## Config file

`dots` searches upward from the current directory for `dots.lua`.

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
dots.symlink("~/.zshrc", ".zshrc")
```

The directory containing `dots.lua` is the project root. Relative source paths
are resolved from that root.

## Resources

- [Symlinks](symlinks.md)
- [Packages](packages.md)

## State

State is stored in `.dots/state.json` inside the project root.

It records which resources are managed by `dots`. This lets `dots` destroy
resources removed from config without touching unmanaged files.

List tracked resources:

```sh
dots state list
```

Stop tracking a resource without changing the filesystem:

```sh
dots state forget ~/.config/nvim
```
