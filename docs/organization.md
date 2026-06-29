# Organizing a dotfiles repo

A small repo can keep everything in `dots.lua`. If you prefer smaller files,
you can separate the config into normal Lua modules and load them with
`require()`.

```lua
require("dots.common")

if dots.platform.family == "arch" then
  require("dots.arch")
end

if dots.platform.family == "darwin" then
  require("dots.macos")
end
```

That maps to this layout:

```txt
dots.lua
dots/
  common.lua
  arch.lua
  macos.lua
```

There is no `dots.load()` wrapper. `dots` adds the repo to Lua's module path, so
plain Lua loading works.

## Common module

One common split is to put shared declarations in `dots/common.lua`:

```lua
dots.user.shell("zsh")

dots.symlink("~/.zshrc", ".zshrc")
dots.symlink("~/.gitconfig", ".gitconfig")

dots.fonts.install()
```

This is usually the boring part of the repo: files, fonts, shell choice, and
anything that is true on every machine.

## Split by platform when the resource is platform-specific

Platform files are useful for package managers, services, or OS-only settings:

```lua
-- dots/arch.lua
dots.pacman.install({ "base-devel", "git", "paru" })
dots.paru.install({ "bat", "docker", "ripgrep" })

dots.user.groups({ "docker" })
dots.systemd.enable({ "docker.service" })
dots.systemd.start({ "docker.service" })
```

```lua
-- dots/macos.lua
dots.brew.install({ "bat", "ripgrep" })
dots.brew.cask({ "ghostty", "obsidian" })
```

This is similar to how Nix configurations are usually organized: a top-level
file imports common modules, then imports system-specific or host-specific
modules. The difference is that `dots` uses plain Lua instead of a module system
of its own.

## Profiles are for machines or personas

Use profiles when the OS is not enough. For example, two Arch machines may need
different Git identities or window-manager config.

```lua
if dots.profile == "work" then
  require("dots.profiles.work")
end

if dots.profile == "personal" then
  require("dots.profiles.personal")
end
```

A useful layout is:

```txt
dots.lua
dots/
  common.lua
  arch.lua
  macos.lua
  profiles/
    work.lua
    personal.lua
```

A practical split is:

- `common.lua`: shared everywhere
- `arch.lua`, `macos.lua`: platform-specific resources
- `profiles/*.lua`: host, role, or persona-specific choices
