# Quick start

## Install

Install the latest release:

```sh
curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

If you're working from a local checkout of `dots`, use the Makefile instead:

```sh
make install
```

## Start a config

From your dotfiles repo, initialize the local files that `dots` needs:

```sh
dots init
```

This creates `dots.lua` if it does not exist and adds `.dots/` to `.gitignore`.
The `.dots` directory holds local state for this machine, so it should not be
committed.

Now declare one file:

```lua
dots.symlink("~/.zshrc", ".zshrc")
```

The first path is where the file should appear in your home directory. The
second path is the file in the repo.

## Check first

Run:

```sh
dots check
```

`dots check` only reads the system and prints what would change. It does not
install packages, start services, copy fonts, or change symlinks.

If `~/.zshrc` is already the right symlink, the output is quiet:

```txt
No changes.
```

On a fresh machine, you might see:

```diff
Symlinks:
  + symlink ~/.zshrc -> .zshrc

Check: 1 to create, 0 to update, 0 to destroy.
```

## Add a package

Add one package for your platform:

```lua
if dots.platform.family == "arch" then
  dots.paru.install({ "ripgrep" })
end

if dots.platform.family == "darwin" then
  dots.brew.install({ "ripgrep" })
end
```

Run another check:

```sh
dots check
```

If the package is missing, the check shows it:

```diff
Packages:
  + paru ripgrep

Check: 1 to create, 0 to update, 0 to destroy.
```

If it is already installed, there may be no visible change. `dots` does not
install anything during check.

## Apply

When the check looks right, apply it:

```sh
dots apply
```

`apply` prints the same check first and asks for confirmation:

```txt
Type 'yes' to apply these changes.
Apply?
```

After a successful apply, `dots` records managed resources in
`.dots/state.json`.
