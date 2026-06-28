# Packages

`dots.paru.install(...)` declares Arch packages managed through `paru`.

```lua
dots.paru.install("bat", "ripgrep", "tmux")
```

You can also pass a Lua table:

```lua
local common_pkg = {
  "bat",
  "btop",
  "fd",
  "ripgrep",
  "tmux",
  "zoxide",
}

dots.paru.install(common_pkg)
```

## Plan behavior

If a package is missing, `dots plan` shows a create action:

```diff
Packages:
  + paru ripgrep

Plan: 1 to create, 0 to update, 0 to destroy.
```

If a managed package is removed from `dots.lua`, `dots plan` shows a destroy
action:

```diff
Packages:
  - paru ripgrep

Plan: 0 to create, 0 to update, 1 to destroy.
```

## Apply behavior

Create uses:

```sh
paru -S --needed <package>
```

Destroy uses:

```sh
paru -Rns <package>
```
