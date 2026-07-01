# State

`dots` keeps local state in the dotfiles repo:

```text
.dots/state.json
```

Do not commit this file. It belongs to one machine.

State is what lets `dots` remove things safely. When `dots check` or
`dots apply` sees that a declared resource already matches the machine, it
records that resource. If the declaration is later removed from `dots.lua`,
`dots check` can show the matching remove without touching unrelated files.

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
group:media
user-group:media
```

## Forget an entry

Use `forget` when something should stay on the machine but stop being managed by
`dots`:

```sh
dots state forget symlink:/home/me/.zshrc
dots state forget package:paru:ripgrep
dots state forget service:systemd:start:docker.service
dots state forget group:media
```

This only edits state. It does not remove files, uninstall packages, or stop
services.
