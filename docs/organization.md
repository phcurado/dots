# Organizing a dotfiles repo

A small setup can live entirely in `dots.lua`. As the repo grows, split the
config into normal Lua modules and load them with `require()`.

```lua
require("dots.common")

if dots.platform.family == "arch" then
  require("dots.arch")
end

if dots.platform.family == "darwin" then
  require("dots.macos")
end
```

One possible layout:

```text
dotfiles/
  dots.lua
  dots/
    common.lua
    arch.lua
    macos.lua
```

There is no `dots.load()` wrapper. `dots` adds the repo to Lua's module path, so
plain Lua loading works.

## Common config

Put declarations shared by every machine in `dots/common.lua`:

```lua
dots.user.shell("zsh")

dots.symlink("~/.zshrc", ".zshrc")
dots.symlink("~/.gitconfig", ".gitconfig")

dots.fonts.install()
```

This is a good place for shared files, fonts, shell settings, and commands.

## Platform config

Put OS-specific declarations in platform modules:

```lua
-- dots/arch.lua
dots.pacman.install({ "base-devel", "git" })
dots.paru.enable({ method = "pacman" })
dots.paru.install({ "bat", "docker", "ripgrep" })

dots.group.create({ "docker" })
dots.user.add_to_groups({ "docker" })
dots.systemd.enable({ "docker.service" })
dots.systemd.start({ "docker.service" })
```

```lua
-- dots/macos.lua
dots.brew.enable()
dots.brew.install({ "bat", "ripgrep" })
dots.brew.cask({ "ghostty", "firefox" })
```

The top-level `dots.lua` stays small: load common config first, then load the
platform module for the current machine.

## Profiles

Profiles are useful when the operating system is not enough. For example, the
same machine might have a work profile and a personal profile.

```lua
if dots.profile == "work" then
  require("dots.profiles.work")
end

if dots.profile == "personal" then
  require("dots.profiles.personal")
end
```

That maps to:

```text
dotfiles/
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
- `arch.lua`, `macos.lua`: OS-specific setup
- `profiles/*.lua`: host, role, or profile-specific setup
