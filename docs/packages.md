# Packages

Package managers are exposed as Lua namespaces:

```lua
dots.pacman.install({ "base-devel", "git" })
dots.paru.install({ "bat", "ripgrep" })
dots.apt.install({ "bat", "ripgrep" })
dots.brew.install({ "bat", "ripgrep" })
```

Built in providers:

- `pacman`
- `paru`
- `apt`
- `brew`

Use platform facts to choose the right provider for the current machine:

```lua
local common_packages = { "bat", "ripgrep" }

if dots.platform.family == "arch" then
  dots.pacman.install({ "base-devel", "git", "paru" })
  dots.paru.install(common_packages)
elseif dots.platform.family == "debian" then
  dots.apt.install(common_packages)
elseif dots.os == "macos" then
  dots.brew.install(common_packages)
  dots.brew.install({ "wget" })
end
```

`family` is derived from `/etc/os-release` on Linux. Arch stays `arch`; Debian,
Ubuntu, and other Debian-like systems use `debian`.

## Shared package lists

Use plain Lua tables for groups you want to reuse:

```lua
local common_packages = {
  "bat",
  "btop",
  "ripgrep",
  "tmux",
  "zoxide",
}

if dots.platform.family == "arch" then
  dots.paru.install(common_packages)
elseif dots.platform.family == "debian" then
  dots.apt.install(common_packages)
elseif dots.os == "macos" then
  dots.brew.install(common_packages)
  dots.brew.install({ "wget" })
end
```

Keep distro-specific package names separate when they differ:

```lua
if dots.platform.family == "arch" then
  dots.paru.install({ "fd" })
elseif dots.platform.family == "debian" then
  dots.apt.install({ "fd-find" })
elseif dots.os == "macos" then
  dots.brew.install({ "fd" })
end
```

## State

Packages are only tracked after `apply`.

If `ripgrep` is already installed and later appears in `dots.lua`, `plan` still
shows it as unmanaged until `apply` records it. That keeps one-off installs from
silently becoming part of the declared setup.

## Plan output

Missing package:

```diff
Packages:
  + paru ripgrep
```

Package removed from config:

```diff
Packages:
  - paru ripgrep
```

## Built-in providers

The built-ins live in Lua, not hardcoded Rust, under `src/lua/prelude.lua`.

`brew` is registered like this:

```lua
dots.provider.package("brew", {
  available = "command -v brew >/dev/null",
  installed = 'brew list --formula "$DOTS_PACKAGE" >/dev/null 2>&1',
  install = 'brew install "$DOTS_PACKAGE"',
  remove = 'brew uninstall "$DOTS_PACKAGE"',
})
```

Each command runs through `sh -c`. The current package name is available as
`DOTS_PACKAGE`.

Provider availability is checked during `apply`, not during `plan`. That allows
bootstrapping flows such as installing `paru` with `pacman` and then using
`paru` later in the same apply.

## Custom providers

Add a provider from `dots.lua` or from any Lua module in the repo:

```lua
dots.provider.package("cargo", {
  available = "command -v cargo >/dev/null",
  installed = "cargo install --list | grep -q \"^$DOTS_PACKAGE \"",
  install = "cargo install \"$DOTS_PACKAGE\"",
  remove = "cargo uninstall \"$DOTS_PACKAGE\"",
})

dots.cargo.install({ "tree-sitter-cli" })
```

For anything more involved, put the logic in a script and call that script from
the provider command.
