# Commands

Commands are resources backed by shell commands. Use them for setup work that
`dots` does not model directly.

```lua
dots.command("oh-my-zsh", {
  check = 'test -d "$HOME/.oh-my-zsh"',
  apply = [[
    sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended
  ]],
})
```

`check` is run during `dots check`. Exit code 0 means the command is already
applied. Any other exit code means it should be applied.

`apply` is run during `dots apply` when `check` fails. After `apply` finishes,
`dots` runs `check` again and reports an error if it still fails.

Commands do not have destroy behavior.

A missing command is shown as:

```diff
Commands:
  + oh-my-zsh
```

## Ordering

The `needs` field controls apply order. Entries in `needs` can be command
references or strings.

A command reference refers to the exact command returned by `dots.command(...)`:

```lua
local node = dots.command("node", {
  check = "command -v node >/dev/null",
  apply = "mise install node@lts",
})

dots.command("prettier", {
  check = "command -v prettier >/dev/null",
  apply = "npm install -g prettier",
  needs = { node },
})
```

The command above installs `prettier` after the `node` command.

A string in `needs` is matched against strings in `provides`:

```lua
dots.command("node", {
  check = "command -v node >/dev/null",
  apply = "mise install node@lts",
  provides = { "node" },
})

dots.command("prettier", {
  check = "command -v prettier >/dev/null",
  apply = "npm install -g prettier",
  needs = { "node" },
})
```

Here, `prettier` needs `node`. Any resource that provides `node` can satisfy
that dependency.

Use command references when depending on a specific command. Use strings when
the dependency is a named thing that may be provided by more than one resource.

`dots` applies resources in dependency order. Dependency cycles are errors.

## Provider helpers

Package and service providers use the same ordering system. The helpers below
make the provider available before resources that need it:

```lua
dots.brew.enable()
dots.brew.install({ "bat", "ripgrep" })
```

```lua
dots.paru.enable({ method = "pacman" })
dots.paru.install({ "bat", "ripgrep" })
```

`dots.brew.enable()` provides the Brew providers used by `dots.brew.*`.
`dots.paru.enable({ method = "pacman" })` declares the `paru` pacman package,
which provides the `paru` provider.
