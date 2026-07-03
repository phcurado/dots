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

## Importing existing files

If a declared target file already exists but the source file is missing from the
repo, `dots check` reports an unmanaged symlink candidate:

```diff
Unmanaged symlink candidates:
  ? ~/.zshrc
    can be imported to .zshrc
```

Review candidates with:

```sh
dots symlink
```

Let `dots` import the files into the repo and link them back with:

```sh
dots symlink apply
```

For directory-style declarations, pass the file to import explicitly:

```sh
dots symlink ~/.config/app/new-file
dots symlink apply ~/.config/app/new-file
```

This keeps broad declarations such as `dots.symlink("~/.config", ".config")`
from offering every generated desktop config file. Explicit imports still respect
`ignore` and only import files whose parent directory already exists in the repo
source tree.

## Conflicts

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
