# State

`dots` keeps local state in the dotfiles repo:

```text
.dots/state.json
```

Do not commit this file. It belongs to one machine.

State is what lets `dots` remove things safely. If `dots` created a symlink,
installed a package, started a service, or copied a font, it records that. If
the declaration is later removed from `dots.lua`, `dots check` can show the
matching remove without touching unrelated files.

## List managed entries

Run:

```sh
dots state list
```

Example entries:

```text
symlink:/home/me/.zshrc
package:paru:ripgrep
service:systemd:start:docker.service
font:/home/me/.local/share/fonts/dots/runcat.ttf
```

## Forget an entry

Use `forget` when something should stay on the machine but stop being managed by
`dots`:

```sh
dots state forget symlink:/home/me/.zshrc
dots state forget package:paru:ripgrep
dots state forget service:systemd:start:docker.service
```

This only edits state. It does not remove files, uninstall packages, or stop
services.
