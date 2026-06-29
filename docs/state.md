# State

`dots` stores local state in the dotfiles repo:

```text
.dots/state.json
```

Do not commit this file. It belongs to one machine.

The state file records what `dots` owns. That is how `dots` can remove a symlink,
uninstall a package, stop a service, or remove a copied font after you delete the
resource from config, without touching unrelated files.

## List resources

Run:

```sh
dots state list
```

You will see entries like:

```text
symlink:/home/me/.zshrc
package:paru:ripgrep
service:systemd:start:docker.service
font:/home/me/.local/share/fonts/dots/runcat.ttf
```

## Forget a resource

Use `forget` when something should stay on the machine but stop being managed by
`dots`:

```sh
dots state forget symlink:/home/me/.zshrc
dots state forget package:paru:ripgrep
dots state forget service:systemd:start:docker.service
```

This only edits state. It does not remove files, uninstall packages, or stop
services.
