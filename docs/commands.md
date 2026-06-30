# Commands

Use commands for setup steps that have a clear check, but do not deserve a
first-class `dots` resource.

For example, you can install Oh My Zsh only when it is missing:

```lua
dots.command("oh-my-zsh", {
  check = 'test -d "$HOME/.oh-my-zsh"',
  apply = [[
    sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended
  ]],
})
```

`check` should return success when the thing is already done. `apply` should do
the work. After applying, `dots` runs `check` again and fails if it still does
not pass.

A missing command looks like this:

```diff
Commands:
  + oh-my-zsh
```

Commands do not have destroy behavior. If you need removal, write it manually.

## Ordering

Commands can depend on other commands:

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

`dots` applies resources in dependency order and reports an error if there is a
cycle.

Commands can also provide capabilities used by other resources:

```lua
dots.command("homebrew", {
  check = "command -v brew >/dev/null",
  apply = [[/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"]],
  provides = { "provider:brew" },
})
```

You usually do not need to write that yourself for built-in providers. Use the
helper instead:

```lua
dots.brew.enable()
dots.brew.install({ "bat", "ripgrep" })
```

On systems where `paru` is available through pacman:

```lua
dots.paru.enable({ method = "pacman" })
dots.paru.install({ "bat", "ripgrep" })
```

`dots.paru.enable({ method = "pacman" })` declares the `paru` pacman package and
makes the `paru` provider available to later package declarations.
