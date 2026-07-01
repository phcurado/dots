# Symlinks

Programs read config from fixed paths in your home directory, such as
`~/.zshrc`, `~/.gitconfig`, or `~/.config/nvim`. A dotfiles repo keeps those
files together. Symlinks connect the paths programs use back to the files in the
repo.

Use `dots.symlink(target, source)`:

```lua
dots.symlink("~/.zshrc", ".zshrc")
dots.symlink("~/.config/nvim", ".config/nvim")
```

The first path is where the program expects the file. The second path is the
file or directory in the repo.

Relative source paths are resolved from the directory containing `dots.lua`, so
the config still works if the repo is cloned somewhere else.

## Files and directories

A single file can be linked directly:

```lua
dots.symlink("~/.gitconfig", ".gitconfig")
dots.symlink("~/.config/starship.toml", ".config/starship.toml")
```

A directory can also be linked:

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
```

Use the shape that matches your repo. `dots` does not require a special layout.

## Home-shaped repos

Some dotfiles repos mirror `$HOME`:

```text
dotfiles/
  .zshrc
  .gitconfig
  .config/
    nvim/
```

For that layout, you can point `~` at the repo root:

```lua
dots.symlink("~", ".", {
  ignore = {
    ".git/**",
    ".jj/**",
    ".dots/**",
    "README.md",
  },
})
```

The `ignore` list keeps repo-only files from being linked into `$HOME`.

If the target directory already exists, `dots` links the children instead of
replacing the directory. For example, if `~/.config` exists and the repo has
`.config/nvim`, `dots` creates the link at `~/.config/nvim`.

## Conflicts

`dots` does not adopt arbitrary files.

If the target already exists and has the same contents as the repo file, `dots`
can replace it with a symlink:

```diff
Symlinks:
  ~ symlink ~/.zshrc -> .zshrc
```

If the target is different, the check reports a conflict:

```diff
Symlinks:
  ! symlink ~/.zshrc (target exists and is not a symlink)
```

Move the file out of the way, or copy the contents into the repo, then run
`dots check` again.

## Removing links

If a link was managed by `dots` and is removed from `dots.lua`, the next check
shows a destroy:

```diff
Symlinks:
  - symlink ~/.zshrc
```

For directory-style declarations, stale cleanup is conservative: `dots` only
removes symlinks that point back into the repo. Regular files created by
applications are left alone.
