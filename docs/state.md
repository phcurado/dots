# State

`dots` keeps state in the dotfiles repo:

```text
.dots/state.json
```

State is local to the machine. Do not commit it.

It records what `dots` owns, so removing something from config can produce a
safe destroy plan without touching unrelated files.

## List resources

```sh
dots state list
```

Example:

```text
symlink:/home/me/.zshrc
package:paru:ripgrep
```

## Forget a resource

Use `forget` when a resource should stay on disk but stop being managed:

```sh
dots state forget symlink:/home/me/.zshrc
dots state forget package:paru:ripgrep
```

This only edits state. It does not remove files or packages.
