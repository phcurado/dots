# Symlinks

Programs usually look for config files in fixed places: `~/.zshrc`,
`~/.gitconfig`, `~/.config/nvim`, and so on. A dotfiles repo keeps those files
in one checkout. Symlinks connect the checkout to the paths programs already
use.

A symlink is declared with `dots.symlink(target, source)`:

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
dots.symlink("~/.zshrc", ".zshrc")
```

The first path is where the link should be created. The second path points to
the file or directory in the repo.

```text
~/.config/nvim -> <repo>/.config/nvim
~/.zshrc       -> <repo>/.zshrc
```

Relative source paths are resolved from the directory that contains `dots.lua`.
That means the config still works if you clone the repo somewhere else.

## Files and directories

A directory can be linked as one unit:

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
```

A file can also be linked directly:

```lua
dots.symlink("~/.gitconfig", ".gitconfig")
dots.symlink("~/.config/starship.toml", ".config/starship.toml")
```

Use whichever shape matches the repo. `dots` does not require a special folder
layout.

## Repos that mirror `$HOME`

Some dotfiles repos are laid out like a home directory. In that case, you can
point `~` at the repo root:

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

The `ignore` list keeps repo-only files from being linked into your home
directory.

`dots` also avoids replacing existing directories. If `~/.config` already exists
and the repo contains `.config/nvim`, `dots` creates this link:

```text
~/.config/nvim -> <repo>/.config/nvim
```

It does not replace the whole `~/.config` directory.

## Conflicts

`dots` will not adopt or overwrite existing files.

If a target already exists and it is not the symlink declared in config, the plan
shows a conflict:

```diff
Symlinks:
  ! symlink ~/.zshrc (target exists and is not a symlink)
```

Move the file yourself, then run `dots plan` again.

## Plan output

A new link looks like this:

```diff
Symlinks:
  + symlink ~/.zshrc -> .zshrc
```

If a managed link points somewhere else, the plan shows an update:

```diff
Symlinks:
  ~ symlink ~/.zshrc -> .zshrc
```

If you remove a link from config, the plan shows a destroy:

```diff
Symlinks:
  - symlink ~/.zshrc
```
