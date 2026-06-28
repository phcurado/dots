# dots documentation

`dots` manages a dotfiles repository from Lua.

## Workflow

Run from the dotfiles repo:

```sh
dots plan
dots apply
```

`plan` shows the changes. `apply` performs them.

## Config file

`dots` searches upward from the current directory for `dots.lua` or
`dots/init.lua`.

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
dots.symlink("~/.zshrc", ".zshrc")
```

## Symlinks

```lua
dots.symlink(target, source)
```

- `target` is where the symlink should exist.
- `source` is the file or directory in the dotfiles repo.
- `~` expands to `$HOME`.
- relative source paths resolve from the repo root.

Example:

```lua
dots.symlink("~/.config/nvim", ".config/nvim")
```

Creates:

```text
~/.config/nvim -> <dotfiles>/.config/nvim
```

## Packages

Package management is planned next. The intended style is direct and explicit:

```lua
dots.pacman.install("bat", "btop", "fd", "ripgrep")
dots.paru.install("neovim-nightly-bin", "noctalia-git")
dots.brew.install("bat", "btop", "fd", "ripgrep")
dots.brew.cask("ghostty", "brave-browser")
```

## Commands

Generic commands will cover tools that do not need first-class providers:

```lua
dots.exec.once("tree-sitter-cli", "cargo install tree-sitter-cli", {
  unless = "command -v tree-sitter",
})
```
