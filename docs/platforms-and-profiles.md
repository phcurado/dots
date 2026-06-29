# Platforms and profiles

A single dotfiles repo can usually cover more than one machine. Most files may
be shared, while package managers, services, or a few personal files vary by
system.

`dots` gives you two ways to branch the config:

- use platform facts for OS and distro decisions
- use profiles for host or persona decisions

For larger configs, keep the branching in small Lua modules. See
[Organizing a dotfiles repo](organization.md).

## Platform

`dots.platform` is available while `dots.lua` is running:

```lua
if dots.platform.system == "x86_64-linux" then
  -- Linux on x86_64
end

if dots.platform.family == "arch" then
  dots.paru.install({ "bat", "ripgrep" })
end

if dots.platform.family == "debian" then
  dots.apt.install({ "bat", "ripgrep" })
end
```

The available fields are:

| Field                    | Example          |
| ------------------------ | ---------------- |
| `dots.platform.system`   | `x86_64-linux`   |
| `dots.platform.arch`     | `x86_64`         |
| `dots.platform.os`       | `linux`          |
| `dots.platform.distro`   | `arch`, `ubuntu` |
| `dots.platform.family`   | `arch`, `debian` |
| `dots.platform.hostname` | `thinkpad`       |

`system` uses the same shape as Nix systems: `<arch>-<os>`. Examples include
`x86_64-linux` and `aarch64-darwin`.

On Linux, `distro` and `family` come from `/etc/os-release`. Ubuntu and Debian
both use `family = "debian"`; Arch uses `family = "arch"`. macOS uses
`family = "darwin"`.

## Profiles

Use profiles when the OS is not enough. For example, two Linux machines might
share packages but use different Git identities.

```lua
if dots.profile == "work" then
  dots.symlink("~/.gitconfig", "profiles/work/gitconfig")
end

if dots.profile == "personal" then
  dots.symlink("~/.gitconfig", "profiles/personal/gitconfig")
end
```

Pass a profile on the command line:

```sh
dots --profile work plan
dots --profile work apply
```

You can also set an environment variable:

```sh
DOTS_PROFILE=work dots plan
```

If neither is set, `dots.profile` defaults to the hostname.
