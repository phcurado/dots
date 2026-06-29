# Symlinks

Symlinks are declared with `dots.symlink(target, source)`:

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
dots.symlink("~/.zshrc", ".zshrc")
```

The first path is where the link should live. The second path is the file or
directory in the repo.

```text
~/.config/nvim -> <repo>/.config/nvim
~/.zshrc       -> <repo>/.zshrc
```

Relative sources are resolved from the project root, so the same config works
from any checkout path.

## Files and directories

Link a whole config directory:

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
```

Or keep it file-by-file:

```lua
dots.symlink("~/.gitconfig", ".gitconfig")
dots.symlink("~/.config/starship.toml", ".config/starship.toml")
```

## Stow-style repos

For repos that mirror `$HOME`, point `~` at the repo root and ignore files that
should stay private to the repo:

```lua
dots.symlink("~", ".", {
  ignore = {
    ".git/**",
    ".jj/**",
    ".dots/**",
    "target/**",
    "README.md",
  },
})
```

Existing directories are preserved. If `~/.config` already exists and the repo
contains `.config/nvim`, `dots` links the child:

```text
~/.config/nvim -> <repo>/.config/nvim
```

It does not replace the whole `~/.config` directory.

## Conflicts

`dots` is conservative. It will not adopt or overwrite existing files.

If a target already exists and is not the symlink declared in config, the plan
shows a conflict:

```diff
Symlinks:
  ! symlink ~/.zshrc (target exists and is not a symlink)
```

Fix the file manually, then run `dots plan` again.

## Plan output

New link:

```diff
Symlinks:
  + symlink ~/.zshrc -> .zshrc
```

Changed managed link:

```diff
Symlinks:
  ~ symlink ~/.zshrc -> .zshrc
```

Link removed from config:

```diff
Symlinks:
  - symlink ~/.zshrc
```
