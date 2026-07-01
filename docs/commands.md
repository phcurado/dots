# Commands

Some setup steps are just shell commands: install a plugin manager, restore a
secret from 1Password, build a small helper, or run a project-specific install
script. Use `dots.command` for those steps.

Each command has a `check` command and an `apply` command:

```lua
dots.command("oh-my-zsh", {
  check = 'test -d "$HOME/.oh-my-zsh"',
  apply = [[
    sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended
  ]],
})
```

`check` runs during `dots check`. Exit code 0 means the setup is already done.
Any other exit code means the command needs to run.

`apply` runs during `dots apply` when `check` fails. After it finishes, `dots`
runs `check` again. If the check still fails, the apply fails.

Commands do not have a remove action. If a setup step needs custom removal,
keep that logic in a script and run it yourself.

## Ordering

Commands can depend on other commands. `dots.command(...)` returns a reference
that can be used in `needs`:

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

This applies `node` before `prettier`.

Strings can also be used with `needs` and `provides`:

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

Here, `prettier` needs a prerequisite named `node`. A command or package that
provides `node` can satisfy it.

Use references when one command depends on another specific command. Use strings
when the dependency is a named prerequisite that could be provided in more than
one way.

Dependency cycles are errors.

## Provider helpers

Provider helpers use the same ordering system. They make a package or service
provider available before other declarations use it:

```lua
dots.brew.enable()
dots.brew.install({ "bat", "ripgrep" })
```

```lua
dots.paru.enable({ method = "pacman" })
dots.paru.install({ "bat", "ripgrep" })
```

`dots.brew.enable()` provides the Homebrew providers used by `dots.brew.*`.
`dots.paru.enable({ method = "pacman" })` installs `paru` through pacman and
provides the `paru` package provider.
