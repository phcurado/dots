<div align="center">

# dots

**Manage your dotfiles across machines seamlessly.**

</div>

## Introduction

`dots` manages machine setup from the dotfiles repository itself. Run it from the
repo, review the plan, then apply the changes.

```sh
cd ~/dotfiles
dots plan
dots apply
```

The config is plain Lua, so it can be split into normal modules and shared across
Linux, macOS, and host-specific files.

## Quick start

Create `dots.lua` in your dotfiles repo:

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
dots.symlink("~/.config/tmux", ".config/tmux")
dots.symlink("~/.zshrc", ".zshrc")
```

Preview the plan:

```sh
dots plan
```

Apply it:

```sh
dots apply
```

## Configuration

`dots` searches upward from the current directory for:

1. `dots.lua`
2. `dots/init.lua`

The directory containing the config is the project root. Relative source paths
are resolved from that root.

A simple cross-machine entrypoint can use normal Lua:

```lua
require("modules.common")

if dots.os == "linux" then
  require("modules.linux")
elseif dots.os == "macos" then
  require("modules.macos")
end
```

## Symlinks

`dots.symlink(target, source)` declares a managed symlink.

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
```

Result:

```text
~/.config/nvim -> <dotfiles>/.config/nvim
```

The source can be a file or a directory. Existing unmanaged targets are reported
as conflicts and are not overwritten.

## Planning

`dots plan` evaluates the Lua config, checks the machine, compares it with local
state, and prints the changes.

```diff
Symlinks:
  + ~/.config/nvim -> ~/dotfiles/.config/nvim
  ~ ~/.zshrc -> ~/dotfiles/.zshrc
  - ~/.config/old
  ! ~/.config/tmux (target exists and is not a symlink)
```

| Marker | Meaning                           |
| ------ | --------------------------------- |
| `+`    | create                            |
| `~`    | update managed symlink            |
| `-`    | remove symlink no longer declared |
| `=`    | already correct                   |
| `!`    | conflict; apply is refused        |

## State

`dots` stores local ownership data in:

```text
.dots/state.json
```

State lets `dots` distinguish managed resources from files created manually.
Only resources recorded in state can be removed automatically.

## More

See [`docs/index.md`](docs/index.md).
