# Symlinks

`dots.symlink(target, source)` declares a symlink managed by `dots`.

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
```

Creates:

```text
~/.config/nvim -> <dotfiles>/.config/nvim
```

## Arguments

| Argument | Description                                                                  |
| -------- | ---------------------------------------------------------------------------- |
| `target` | Path where the symlink should exist. `~` expands to `$HOME`.                 |
| `source` | File or directory to point to. Relative paths resolve from the project root. |

## Files and directories

The source can be a file:

```lua
dots.symlink("~/.zshrc", ".zshrc")
```

Or a directory:

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
```

## Stow-style directories

A directory can be expanded into symlinks for its children by passing `ignore`.
This keeps existing parent directories, like `~/.config`, and links the entries
inside them.

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

With a repo containing `.config/nvim`, this declares:

```text
~/.config/nvim -> <dotfiles>/.config/nvim
```

not:

```text
~/.config -> <dotfiles>/.config
```

`dots` descends into source directories when the matching target directory
already exists. Otherwise it links the entry directly.

## Plan behavior

If the target is missing, `dots plan` shows a create action:

```diff
Symlinks:
  + symlink ~/.zshrc -> .zshrc

Plan: 1 to create, 0 to update, 0 to destroy.
```

If a managed symlink points to a different source, the plan shows an update:

```diff
Symlinks:
  ~ symlink ~/.zshrc -> .zshrc

Plan: 0 to create, 1 to update, 0 to destroy.
```

If a managed symlink is removed from `dots.lua`, the plan shows a destroy action:

```diff
Symlinks:
  - symlink ~/.zshrc

Plan: 0 to create, 0 to update, 1 to destroy.
```

If the target exists and cannot be safely managed, the plan shows a conflict and
`dots apply` is refused.

```diff
Symlinks:
  ! symlink ~/.zshrc (target exists and is not a symlink)

Plan: 0 to create, 0 to update, 0 to destroy, 1 conflict
```
