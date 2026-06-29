# dots

`dots` gives a dotfiles repo a plan/apply workflow. Declare the setup you want,
preview the changes, then apply them when the plan looks right.

The docs match the current `main` branch.

## Start here

- [Quick start](quick-start.md): install `dots`, create a config, and run your first plan.
- [Install](install.md): install script and source install.
- [Platforms and profiles](platforms-and-profiles.md): target systems, distros, hosts, and profiles.
- [Symlinks](symlinks.md): link files and directories from the repo into `$HOME`.
- [Packages](packages.md): use `pacman`, `paru`, `apt`, `brew`, or your own package provider.
- [Services](services.md): manage systemd and Homebrew services.
- [Fonts](fonts.md): install local fonts on Linux and macOS.
- [State](state.md): inspect or forget resources tracked by `dots`.
- [Release](release.md): tag-based GitHub releases.

## Workflow

Run commands from inside the dotfiles repo:

```sh
dots plan
dots apply
```

`plan` is the dry run. `apply` performs the changes and updates local state.
