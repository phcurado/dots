# Packages

Package declarations use provider namespaces.

```lua
dots.paru.install({ "bat", "ripgrep" })
dots.pacman.install({ "base-devel", "git" })
dots.apt.install({ "bat", "ripgrep" })
```

The built-in package providers are:

- `paru`
- `pacman`
- `apt`

## Tables

Package install takes a Lua table:

```lua
local common_pkg = {
  "bat",
  "btop",
  "fd",
  "ripgrep",
  "tmux",
  "zoxide",
}

dots.paru.install(common_pkg)
```

## Plan behavior

If a package is missing, `dots plan` shows a create action:

```diff
Packages:
  + paru ripgrep

Plan: 1 to create, 0 to update, 0 to destroy.
```

If a managed package is removed from `dots.lua`, `dots plan` shows a destroy
action:

```diff
Packages:
  - paru ripgrep

Plan: 0 to create, 0 to update, 1 to destroy.
```

Installed packages are not tracked by `dots plan` alone. Run `dots apply` to
record declared packages as managed.

## Custom providers

Package providers can be defined in Lua:

```lua
dots.provider.package("cargo", {
  available = "command -v cargo >/dev/null",
  installed = "cargo install --list | grep -q \"^$DOTS_PACKAGE \"",
  install = "cargo install \"$DOTS_PACKAGE\"",
  remove = "cargo uninstall \"$DOTS_PACKAGE\"",
})

dots.cargo.install({ "tree-sitter-cli" })
```

Provider commands run through `sh -c`. The package name is available as the
`DOTS_PACKAGE` environment variable.
