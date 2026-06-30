# Fonts

Keep font files in the repo and let `dots` install them for the current OS. On
Linux, that means copying them to your user font directory and refreshing
fontconfig. On macOS, it means copying them to your user Fonts folder.

The default layout is:

```text
fonts/
  0xProtoNerdFont-Regular.ttf
  0xProtoNerdFont-Bold.ttf
```

With that layout, add this to `dots.lua`:

```lua
dots.fonts.install()
```

If you use a different directory, pass it explicitly:

```lua
dots.fonts.install("assets/fonts")
```

## What gets installed

`dots` walks the font directory and installs files with these extensions:

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

After apply, `dots` runs:

```sh
fc-cache -f ~/.local/share/fonts/dots
```

## macOS

On macOS, fonts are copied to:

```text
~/Library/Fonts/dots/
```

No extra cache command is needed.

## Check output

A new font looks like this:

```diff
Fonts:
  + ~/.local/share/fonts/dots/0xProtoNerdFont-Regular.ttf
```

If you remove a font from the repo, `dots check` shows a destroy for the copied
font file.
