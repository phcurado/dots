<div align="center">

# dots

**Declarative dotfiles and workstation setup.**

**[Docs](docs/index.md) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Quick start](docs/quick-start.md) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Symlinks](docs/symlinks.md) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Packages](docs/packages.md)**

</div>

## Introduction

`dots` brings a Terraform-style workflow to a dotfiles repo: declare the setup,
review the plan, then apply it. It manages symlinks, package installs, and local
state without taking over files it does not own.

## Quick start

In your dotfiles repo, add a `dots.lua` file:

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

Check the plan:

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

Apply it:

```sh
dots --profile work apply
```

```text
Applying changes...

  symlink.~/.config/nvim: Creating...
  symlink.~/.config/nvim: Create complete
  symlink.~/.config/tmux: Creating...
  symlink.~/.config/tmux: Create complete
  symlink.~/.zshrc: Creating...
  symlink.~/.zshrc: Create complete
  symlink.~/.gitconfig: Creating...
  symlink.~/.gitconfig: Create complete
  package.pacman.base-devel: Installing...
  package.pacman.base-devel: Install complete
  package.pacman.git: Installing...
  package.pacman.git: Install complete
  package.pacman.paru: Installing...
  package.pacman.paru: Install complete
  package.paru.bat: Installing...
  package.paru.bat: Install complete
  package.paru.ripgrep: Installing...
  package.paru.ripgrep: Install complete

Apply complete: 9 created, 0 updated, 0 destroyed.
```

For the full config API, state commands, and provider examples, see the
[docs](docs/index.md).
