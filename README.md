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

`dots` lets you run a dotfiles repo with a check/apply workflow. Describe the
setup you want, inspect the diff with `dots check`, then apply it with
`dots apply`.

You can start small. Keep your current repo layout, move one thing at a time,
and let `dots` manage only the parts you declare: symlinks, packages, services,
fonts, user settings, and local state.

## Quick start

Install the latest release:

```sh
curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

Now create a dotfiles repo, or use the one you already have:

```sh
dots init
```

Add one file to `dots.lua`:

```lua
dots.symlink("~/.zshrc", ".zshrc")
```

Check what would happen:

```sh
dots check
```

On a fresh machine, the check shows a create:

```diff
Symlinks:
  + symlink ~/.zshrc -> .zshrc

Check: 1 to create, 0 to update, 0 to destroy.
```

Add packages when you're ready:

```lua
if dots.platform.family == "arch" then
  dots.paru.enable({ method = "pacman" })
  dots.paru.install({ "bat", "ripgrep" })
end

if dots.platform.family == "darwin" then
  dots.brew.enable()
  dots.brew.install({ "bat", "ripgrep" })
end
```

If the check looks right, apply it:

```sh
dots apply
```

See the [docs](docs/index.md) for symlinks, packages, services, fonts, user
settings, profiles, and state.
