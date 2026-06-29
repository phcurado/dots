# Packages

Package managers are exposed as Lua namespaces:

```lua
dots.pacman.install({ "base-devel", "git" })
dots.paru.install({ "bat", "ripgrep" })
dots.apt.install({ "bat", "ripgrep" })
```

Built in providers:

- `pacman`
- `paru`
- `apt`

Use platform facts to choose the right provider for the current machine:

```lua
if dots.platform.family == "arch" then
  dots.pacman.install({ "base-devel", "git", "paru" })
  dots.paru.install({ "bat", "fd", "ripgrep" })
elseif dots.platform.family == "debian" then
  dots.apt.install({ "bat", "fd-find", "ripgrep" })
end
```

`family` is derived from `/etc/os-release` on Linux. Arch stays `arch`; Debian,
Ubuntu, and other Debian-like systems use `debian`.

## Shared package lists

Use plain Lua tables for groups you want to reuse:

```lua
local cli = {
  "bat",
  "btop",
  "fd",
  "ripgrep",
  "tmux",
  "zoxide",
}

if dots.platform.family == "arch" then
  dots.paru.install(cli)
elseif dots.platform.family == "debian" then
  dots.apt.install({ "bat", "btop", "fd-find", "ripgrep", "tmux", "zoxide" })
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

`paru` is registered like this:

```lua
dots.provider.package("paru", {
  available = "command -v paru >/dev/null",
  installed = "paru -Q \"$DOTS_PACKAGE\" >/dev/null 2>&1",
  install = "paru -S --needed \"$DOTS_PACKAGE\"",
  remove = "paru -Rns \"$DOTS_PACKAGE\"",
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
