# Packages

A dotfiles setup often expects the same tools to exist on every machine: `git`,
`ripgrep`, `bat`, `neovim`, and so on. `dots` can install those OS packages as
part of the same config that manages the files.

Choose the package manager by using its namespace:

```lua
dots.pacman.install({ "base-devel", "git" })
dots.paru.enable({ method = "pacman" })
dots.paru.install({ "bat", "ripgrep" })

dots.apt.install({ "bat", "ripgrep" })

dots.brew.enable()
dots.brew.install({ "bat", "ripgrep" })
dots.brew.cask({ "firefox" })
```

The built-in providers are:

- `pacman`
- `paru`
- `apt`
- `brew`
- `brew-cask`, exposed as `dots.brew.cask(...)`
- `brew-tap`, exposed as `dots.brew.tap(...)`

`dots.brew.enable()` handles Homebrew when it is missing. `dots.paru.enable({
method = "pacman" })` installs `paru` with pacman, so later `dots.paru.install`
declarations can use it.

## Platform-specific packages

Different operating systems use different package managers. Sometimes the same
tool also has a different package name. Keep that logic in Lua:

```lua
local common_packages = { "bat", "ripgrep" }

if dots.platform.family == "arch" then
  dots.pacman.install({ "base-devel", "git" })
  dots.paru.enable({ method = "pacman" })
  dots.paru.install(common_packages)
end

if dots.platform.family == "darwin" then
  dots.brew.enable()
  dots.brew.install(common_packages)
  dots.brew.cask({ "firefox" })
end
```

Do not force everything into a shared table. If package names differ, keep them
separate:

```lua
if dots.platform.family == "arch" then
  dots.paru.install({ "fd" })
end

if dots.platform.family == "debian" then
  dots.apt.install({ "fd-find" })
end
```

On Linux, `dots.platform.family` comes from `/etc/os-release`. Arch uses
`family = "arch"`; Debian and Ubuntu use `family = "debian"`; macOS uses
`family = "darwin"`.

## Check and apply

A missing package appears in the check output:

```diff
Packages:
  + paru ripgrep
```

If a managed package is removed from `dots.lua`, the check shows the remove:

```diff
Packages:
  - paru ripgrep
```

Packages are recorded in state after `dots apply`, not after `dots check`. If a
package is already installed before `dots` manages it, apply records it without
reinstalling it. This keeps random one-off installs from silently becoming part
of the managed setup during check.

## Custom providers

Package providers are Lua definitions. A provider needs commands for checking
whether the package manager exists, checking whether one package is installed,
installing it, and removing it.

For example, a small Cargo provider can live in `dots.lua` or in a Lua module:

```lua
dots.provider.package("cargo", {
  available = "command -v cargo >/dev/null",
  installed = "cargo install --list | grep -q \"^$DOTS_PACKAGE \"",
  install = "cargo install \"$DOTS_PACKAGE\"",
  remove = "cargo uninstall \"$DOTS_PACKAGE\"",
})

dots.cargo.install({ "tree-sitter-cli" })
```

Provider commands run through `sh -c`. The package name is available as
`DOTS_PACKAGE`.

If a package manager needs more logic than fits in one line, put the logic in a
script and call the script from the provider command.
