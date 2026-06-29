# Quick start

## Install

From this repo:

```sh
make install
```

This installs `dots` to `~/.local/bin/dots`.

## Create a config

In your dotfiles repo, add `dots.lua`:

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
dots.symlink("~/.config/tmux", ".config/tmux")
dots.symlink("~/.zshrc", ".zshrc")

if dots.platform.family == "arch" then
  dots.pacman.install({ "base-devel", "git", "paru" })
  dots.paru.install({ "bat", "ripgrep" })
elseif dots.platform.family == "debian" then
  dots.apt.install({ "bat", "ripgrep" })
end

if dots.profile == "work" then
  dots.symlink("~/.gitconfig", "profiles/work/gitconfig")
end
```

`dots.platform.family` is for system choices such as Arch vs Debian.
`dots.profile` is for machine or persona choices. By default it uses the
hostname, but you can pass one explicitly.

## Plan

```sh
dots --profile work plan
```

```diff
Initializing state: .dots/state.json

Symlinks:
  + symlink ~/.config/nvim -> .config/nvim
  + symlink ~/.config/tmux -> .config/tmux
  + symlink ~/.zshrc -> .zshrc
  + symlink ~/.gitconfig -> profiles/work/gitconfig

Packages:
  + pacman base-devel
  + pacman git
  + pacman paru
  + paru bat
  + paru ripgrep

Plan: 9 to create, 0 to update, 0 to destroy.
```

## Apply

```sh
dots --profile work apply
```

After apply, `dots` records the managed resources in `.dots/state.json`.
