# Fonts

Terminal and editor configs often assume a specific font is installed. `dots`
can keep those font files in the repo and install them for the current operating
system.

The default location is `fonts/`:

```text
dotfiles/
  dots.lua
  fonts/
    0xProtoNerdFont-Regular.ttf
    0xProtoNerdFont-Bold.ttf
```

With that layout, add this to `dots.lua`:

```lua
dots.fonts.install()
```

A different directory can be passed explicitly:

```lua
dots.fonts.install("assets/fonts")
```

## Installed files

`dots` installs files with these extensions:

- `.ttf`
- `.otf`
- `.ttc`
- `.otc`

Other files are ignored, so license files and notes can stay next to the fonts.

## Linux

On Linux, fonts are copied to:

```text
~/.local/share/fonts/dots/
```

After applying font changes, `dots` refreshes fontconfig:

```sh
fc-cache -f ~/.local/share/fonts/dots
```

## macOS

On macOS, fonts are copied to:

```text
~/Library/Fonts/dots/
```

No cache command is needed.

## Check output

A new font appears as:

```diff
Fonts:
  + ~/.local/share/fonts/dots/0xProtoNerdFont-Regular.ttf
```

If a managed font is removed from the repo, `dots check` shows a destroy for the
installed copy. If the installed copy changed outside dots, removal is refused.
