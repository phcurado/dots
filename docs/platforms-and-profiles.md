# Platforms and profiles

A single dotfiles repo can be used on different operating systems and in
different modes. Arch and macOS use different package managers. Two machines may
need different services. One laptop may have both a personal profile and a work
profile.

`dots` exposes platform facts while `dots.lua` runs. It also exposes
`dots.profile`, which can be selected from the command line.

## Platform facts

Use `dots.platform` for OS and distro decisions:

```lua
if dots.platform.family == "arch" then
  dots.yay.enable({ method = "aur" })
  dots.yay.install({ "bat", "ripgrep" })
end

if dots.platform.family == "darwin" then
  dots.brew.enable()
  dots.brew.install({ "bat", "ripgrep" })
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

`system` uses the Nix-style shape `<arch>-<os>`, such as `x86_64-linux` or
`aarch64-darwin`.

`family` values:

| Distros                           | `family` | Packages      |
| --------------------------------- | -------- | ------------- |
| Arch, EndeavourOS                 | `arch`   | `dots.pacman` |
| Debian, Ubuntu, Pop!_OS           | `debian` | `dots.apt`    |
| Fedora, RHEL, CentOS, Rocky, Alma | `fedora` | `dots.dnf`    |
| openSUSE, SLES                    | `suse`   | `dots.zypper` |
| Alpine                            | `alpine` | `dots.apk`    |
| macOS                             | `darwin` | `dots.brew`   |

## Profiles

Use profiles for choices that are not determined by the operating system. For
example, the same laptop can use a work Git config during the day and a personal
Git config outside work:

```lua
if dots.profile == "work" then
  dots.symlink("~/.gitconfig", "profiles/work/gitconfig")
end

if dots.profile == "personal" then
  dots.symlink("~/.gitconfig", "profiles/personal/gitconfig")
end
```

Select a profile on the command line:

```sh
dots --profile work check
dots --profile work apply
```

Or with an environment variable:

```sh
DOTS_PROFILE=work dots check
```

If no profile is selected, `dots.profile` defaults to the hostname.

Profiles can also control services:

```lua
if dots.profile == "work" then
  dots.systemd.start({ "example-vpn.service" })
end
```
