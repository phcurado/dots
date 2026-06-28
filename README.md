<div align="center">

# dots

**Manage your dotfiles across machines seamlessly.**

</div>

`dots` runs from a dotfiles repository, fetches configuration, shows the planned changes and applies the state diff to keep your setup declarative and unified.

## Quick start

Create `dots.lua` in your dotfiles directory:

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
dots.symlink("~/.config/tmux", ".config/tmux")
dots.symlink("~/.zshrc", ".zshrc")
```

Preview:

```sh
dots plan
```

Example output:

```diff
Initializing state: .dots/state.json

Symlinks:
  + symlink ~/.config/nvim -> .config/nvim
  + symlink ~/.config/tmux -> .config/tmux
  + symlink ~/.zshrc -> .zshrc

Plan: 3 to create, 0 to update, 0 to destroy.
```

Apply:

```sh
dots apply
```

## Configuration

`dots` searches upward from the current directory for `dots.lua`. The directory containing `dots.lua` is the project root, and relative source paths are resolved from there.

Split configuration with normal Lua modules:

```lua
require("modules.common")
require("modules.linux")
```

## State

Local state lives in:

```text
.dots/state.json
```

State records ownership so `dots` can remove managed resources without touching unmanaged files.

## Documentation

- [Symlinks](docs/symlinks.md)
- [Packages](docs/packages.md)
- [Full docs](docs/index.md)
