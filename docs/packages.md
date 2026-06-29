# Packages

A dotfiles repo often assumes a few tools are installed. Your shell might call
`fzf`, Neovim might use `ripgrep`, and a macOS setup might need Homebrew casks.
`dots` lets you declare those packages in the same place as the rest of the
machine setup.

Package managers are available as namespaces:

```lua
dots.pacman.install({ "base-devel", "git", "paru" })
dots.paru.install({ "bat", "ripgrep" })
dots.apt.install({ "bat", "ripgrep" })
dots.brew.install({ "bat", "ripgrep" })
dots.brew.cask({ "ghostty" })
```

The built-in providers are:

- `pacman`
- `paru`
- `apt`
- `brew`
- `brew-cask`, exposed as `dots.brew.cask(...)`
- `brew-tap`, exposed as `dots.brew.tap(...)`

## Choosing packages by platform

Let's say the same repo is used on Arch and macOS. You can keep common packages
in one table and add platform-specific packages next to them:

```lua
local common_packages = { "bat", "ripgrep" }

if dots.platform.family == "arch" then
  dots.pacman.install({ "base-devel", "git", "paru" })
  dots.paru.install(common_packages)
elseif dots.os == "macos" then
  dots.brew.tap({ "FelixKratz/formulae" })
  dots.brew.install(common_packages)
  dots.brew.install({ "wget", "sketchybar" })
  dots.brew.cask({ "ghostty" })
end
```

On Linux, `dots.platform.family` comes from `/etc/os-release`. Arch gets
`family = "arch"`; Debian and Ubuntu get `family = "debian"`.

## When package names differ

Some package names are not portable. Keep those declarations separate:

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

Packages are tracked after `apply`, not after `plan`.

For example, if `ripgrep` is already installed and you add it to `dots.lua`, the
first plan still treats it as a declared resource that needs to be recorded. The
apply step records it in state. This keeps one-off local installs from silently
becoming part of the managed setup.

## Plan output

A missing package looks like this:

```diff
Packages:
  + paru ripgrep
```

If a managed package is removed from config, the plan shows a destroy:

```diff
Packages:
  - paru ripgrep
```

## Built-in providers

The built-ins are Lua files under `src/lua/packages/`.

For example, the Homebrew formula provider is registered like this:

```lua
dots.provider.package("brew", {
  available = "command -v brew >/dev/null",
  installed = 'brew list --formula "$DOTS_PACKAGE" >/dev/null 2>&1',
  install = 'brew install "$DOTS_PACKAGE"',
  remove = 'brew uninstall "$DOTS_PACKAGE"',
})
```

Casks use a separate provider and a shorter helper:

```lua
dots.provider.package("brew-cask", {
  available = "command -v brew >/dev/null",
  installed = 'brew list --cask "$DOTS_PACKAGE" >/dev/null 2>&1',
  install = 'brew install --cask "$DOTS_PACKAGE"',
  remove = 'brew uninstall --cask "$DOTS_PACKAGE"',
})

dots.brew.cask = dots["brew-cask"].install
```

Provider commands run through `sh -c`. The package name is passed in the
`DOTS_PACKAGE` environment variable.

Provider availability is checked during `apply`, not during `plan`. That makes
bootstrap flows possible. For example, an Arch config can install `paru` with
`pacman` and use `paru` later in the same apply.

## Custom providers

A simple provider can live in `dots.lua` or in a Lua module in the repo:

```lua
dots.provider.package("cargo", {
  available = "command -v cargo >/dev/null",
  installed = "cargo install --list | grep -q \"^$DOTS_PACKAGE \"",
  install = "cargo install \"$DOTS_PACKAGE\"",
  remove = "cargo uninstall \"$DOTS_PACKAGE\"",
})

dots.cargo.install({ "tree-sitter-cli" })
```

If a package manager needs more logic, put that logic in a script and call the
script from the provider command.
