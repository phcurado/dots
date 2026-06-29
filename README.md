<div align="center">

# dots

**Manage your dotfiles declaratively, without configuring each machine by hand**

**[Docs](docs/index.md) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Quick start](docs/quick-start.md) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Install](docs/install.md) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Symlinks](docs/symlinks.md) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Packages](docs/packages.md)**

</div>

## Introduction

`dots` lets you run a dotfiles repo with a plan/apply workflow. Describe the
setup you want, check the diff with `dots plan`, then apply it with
`dots apply`.

You can start small. Keep your current repo layout, move one thing at a time,
and let `dots` manage only the parts you declare: symlinks, packages, services,
fonts, and local state.

## Quick start

Install the latest release:

```sh
curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

Now create a dotfiles repo, or use the one you already have, and add `dots.lua`:

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

Run a plan:

```sh
dots plan
```

On a fresh machine, you should see something like this:

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

If the plan looks right, apply it:

```sh
dots apply
```

See the [docs](docs/index.md) for symlinks, packages, services, fonts, profiles,
and state.
