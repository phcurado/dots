# Platforms and profiles

Use platform facts for operating-system decisions, and profiles for host or
persona decisions.

## Platform

`dots.platform` is detected when the config is loaded:

```lua
if dots.platform.system == "x86_64-linux" then
  -- Linux on x86_64
end

if dots.platform.family == "arch" then
  dots.paru.install({ "bat", "ripgrep" })
elseif dots.platform.family == "debian" then
  dots.apt.install({ "bat", "ripgrep" })
end
```

Available fields:

| Field                    | Example          |
| ------------------------ | ---------------- |
| `dots.platform.system`   | `x86_64-linux`   |
| `dots.platform.arch`     | `x86_64`         |
| `dots.platform.os`       | `linux`          |
| `dots.platform.distro`   | `arch`, `ubuntu` |
| `dots.platform.family`   | `arch`, `debian` |
| `dots.platform.hostname` | `thinkpad`       |

`system` follows the Nix-style `<arch>-<os>` shape, such as `x86_64-linux` or
`aarch64-darwin`.

On Linux, `distro` and `family` come from `/etc/os-release`. Ubuntu and Debian
both use `family = "debian"`; Arch uses `family = "arch"`.

## Profiles

Profiles are explicit config targets:

```lua
if dots.profile == "work" then
  dots.symlink("~/.gitconfig", "profiles/work/gitconfig")
elseif dots.profile == "personal" then
  dots.symlink("~/.gitconfig", "profiles/personal/gitconfig")
end
```

Pass a profile on the command line:

```sh
dots --profile work plan
dots --profile work apply
```

Or use an environment variable:

```sh
DOTS_PROFILE=work dots plan
```

If neither is set, `dots.profile` defaults to the hostname.
