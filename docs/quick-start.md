# Quick start

Let's create a small config and ask `dots` what it would do. You can do this in
a new dotfiles repo or in a repo you already use.

## Install

Install the latest release:

```sh
curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

If you're working from a local checkout of `dots`, use the Makefile instead:

```sh
make install
```

## Create a config

Add `dots.lua` to your dotfiles repo:

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
dots.symlink("~/.config/tmux", ".config/tmux")
dots.symlink("~/.zshrc", ".zshrc")

local common_packages = { "bat", "ripgrep" }

if dots.platform.family == "arch" then
  dots.pacman.install({ "base-devel", "git", "paru" })
  dots.paru.install(common_packages)
end

if dots.platform.family == "darwin" then
  dots.brew.install(common_packages)
  dots.brew.install({ "wget" })
end
```

The symlink lines say where files from the repo should appear in your home
directory. The package block uses platform facts so the same config can run on
Arch and macOS.

## Run a plan

Run this from inside the dotfiles repo:

```sh
dots plan
```

On a fresh machine, the output should look similar to this:

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

If a file already exists where a symlink should go, `dots` will report a
conflict instead of overwriting it.

## Apply the plan

When the plan looks right, apply it:

```sh
dots apply
```

`dots` writes `.dots/state.json` in the repo after applying. Keep that file local
to the machine.
