<div align="center">

# dots

<p><img title="dots logo" src="logo.png" width="360" alt="dots logo"></p>

**Manage dotfiles declaratively and reuse your setup across machines**

**[Docs](https://phcurado.github.io/dots/) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Quick start](https://phcurado.github.io/dots/quick-start) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Install](https://phcurado.github.io/dots/install) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Changelog](https://github.com/phcurado/dots/blob/main/CHANGELOG.md) &nbsp;&nbsp;•&nbsp;&nbsp;**
**[Release](https://github.com/phcurado/dots/releases)**

</div>

## Introduction

`dots` helps you manage machine configuration. Create symlinks, install
packages, start and enable services, install fonts, and run commands in a
declarative way.

Use it for your computers, dotfiles, or servers. Declare the setup once and reuse
it across your machines.

## Quick start

Install the latest release:

```sh
curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

Create a dotfiles repo, or use one you already have:

```sh
dots init
```

The command above creates the file `dots.lua`, the main entrypoint for your configuration.
You can manage symlinks, packages, services, and more:

```lua
local packages = { "bat", "ripgrep" }

dots.symlink("~/.zshrc", ".zshrc")

if dots.platform.family == "arch" then
  dots.pacman.install({ "base-devel", "git" })
  dots.yay.enable({ method = "aur" })
  dots.yay.install(packages)
  dots.systemd.enable({ "docker.service" })
  dots.systemd.start({ "docker.service" })
end

if dots.platform.family == "darwin" then
  dots.brew.enable()
  dots.brew.install(packages)
  dots.brew.cask({ "firefox" })
end
```

Plan the changes:

```sh
dots check
```

On a fresh Arch distro, it should show:

```diff
Symlinks:
+ symlink ~/.zshrc -> .zshrc

Packages:
+ pacman base-devel
+ pacman git
+ yay bat
+ yay ripgrep

Services:
+ systemd enable docker.service
+ systemd start docker.service

Check: 7 to create, 0 to update, 0 to destroy.
```

Apply the changes:

```sh
dots apply
```

`dots` will create the symlinks, install the packages, and start the services you declared.
You declare what the system should have and how it should behave, then reuse the
same setup across the machines that need it.

See the [docs](https://phcurado.github.io/dots/) for more.
