# Fonts

Put local fonts in the repo under `fonts/`:

```text
fonts/
  0xProtoNerdFont-Regular.ttf
  0xProtoNerdFont-Bold.ttf
```

Then add this to `dots.lua`:

```lua
dots.fonts.install()
```

If you keep fonts somewhere else, pass the directory:

```lua
dots.fonts.install("assets/fonts")
```

`dots` copies fonts instead of symlinking them, so the same config works on
Linux and macOS.

## Supported files

`dots` looks for font files under the directory you pass. If that directory has
nested folders, it walks through those too.

Supported extensions:

- `.ttf`
- `.otf`
- `.ttc`
- `.otc`

## Linux

Fonts are copied to:

```text
~/.local/share/fonts/dots/
```

After apply, `dots` refreshes the font cache:

```sh
fc-cache -f ~/.local/share/fonts/dots
```

## macOS

Fonts are copied to:

```text
~/Library/Fonts/dots/
```

## Plan output

```diff
Fonts:
  + ~/.local/share/fonts/dots/0xProtoNerdFont-Regular.ttf
```

If you remove a font from the repo, `dots plan` shows a destroy for the copied
font file.
