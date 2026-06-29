# Quick start

## Install

Install the latest release:

```sh
curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

From source:

```sh
make install
```

## Create a config

Create a dotfiles repo, or use your existing one, and add `dots.lua`:

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
dots.symlink("~/.config/tmux", ".config/tmux")
dots.symlink("~/.zshrc", ".zshrc")

local common_packages = { "bat", "ripgrep" }

if dots.platform.family == "arch" then
  dots.pacman.install({ "base-devel", "git", "paru" })
  dots.paru.install(common_packages)
elseif dots.os == "macos" then
  dots.brew.install(common_packages)
  dots.brew.install({ "wget" })
end
```

Use platform checks when the same repo is shared across machines.

## Plan

```sh
dots plan
```

```diff
Initializing state: .dots/state.json

Symlinks:
  + symlink ~/.config/nvim -> .config/nvim
  + symlink ~/.config/tmux -> .config/tmux
  + symlink ~/.zshrc -> .zshrc

Packages:
  + pacman base-devel
  + pacman git
  + pacman paru
  + paru bat
  + paru ripgrep

Plan: 8 to create, 0 to update, 0 to destroy.
```

## Apply

```sh
dots apply
```

After apply, `dots` records the managed resources in `.dots/state.json`.
