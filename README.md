<div align="center">

# dots

**Manage your dotfiles declaratively, never waste time configuring your OS again**

**[Docs](docs/index.md) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Quick start](docs/quick-start.md) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Install](docs/install.md) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Symlinks](docs/symlinks.md) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Packages](docs/packages.md)**

</div>

## Introduction

`dots` brings a Terraform-style workflow to a dotfiles repo: declare the setup,
review the plan, then apply it. It manages symlinks, package installs, and local
state without taking over files it does not own.

## Quick start

Install the latest release:

```sh
curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

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

Check the plan:

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

Apply it:

```sh
dots apply
```

For the full config API, state commands, and provider examples, see the
[docs](docs/index.md).
