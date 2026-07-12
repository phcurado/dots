# Files

Manage a regular file in your dots project:

```lua
dots.file("~/.ssh/config", {
  source = "ssh/config",
  mode = "0600",
})
```

When mode is omitted, dots preserves the mode of an existing target. A new target uses the platform's default file mode.

## Existing files

If an existing target has the same content and mode, dots adopts it without rewriting it. A different unmanaged target is reported as a conflict.

Once tracked, changes to the source update the target. Mode-only changes update permissions without rewriting the file.

Only regular files are supported. Directories, symlinks and special files at the source or target are reported as conflicts.

## Removing a declaration

Removing `dots.file` from the configuration stops tracking the target but does not delete it.
