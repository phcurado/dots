<div align="center">

# dots

<p><img title="dots logo" src="https://raw.githubusercontent.com/phcurado/dots/main/logo.png" width="360" alt="dots logo"></p>

**Manage your dotfiles declaratively, without configuring each machine by hand**

**[Docs](https://phcurado.github.io/dots/) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Quick start](https://phcurado.github.io/dots/quick-start) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Install](https://phcurado.github.io/dots/install) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Release](https://github.com/phcurado/dots/releases)**

</div>

## Introduction

`dots` helps manage a dotfiles repo across machines. It can create symlinks,
install OS packages, start services, copy fonts, and run checked setup commands.

You can use it with an existing repo or start from scratch. Keep your current
layout, move one piece at a time, and let `dots` manage only the parts you add
to `dots.lua`.

## Quick start

Install the latest release:

```sh
curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

Create a dotfiles repo, or use one you already have:

```sh
dots init
```

Add a symlink to `dots.lua`:

```lua
dots.symlink("~/.zshrc", ".zshrc")
```

Check the diff:

```sh
dots # or dots check
```

On a fresh machine, the check shows a create:

```diff
Symlinks:
  + symlink ~/.zshrc -> .zshrc

Check: 1 to create, 0 to update, 0 to destroy.
```

Add packages when you are ready:

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

If the diff looks right, apply it:

```sh
dots apply
```

See the [docs](docs/index.md) for symlinks, packages, services, fonts, user
settings, profiles, and state.
