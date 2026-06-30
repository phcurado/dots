# dots

`dots` is meant to be run from inside your dotfiles repo. The repo stays the
source of truth: you describe the setup you want, check what would change, and
only then apply it.

```sh
dots check
dots apply
```

Running `dots` with no command is the same as `dots check`.

If you're trying `dots` on a repo you already use, start with `dots check`. It
will show the diff without installing packages, starting services, copying fonts,
or changing symlinks.

## Start here

- [Quick start](quick-start.md): create a small config and run the first check.
- [Install](install.md): install from a release or from source.
- [Organization](organization.md): split a config with normal Lua modules.
- [Platforms and profiles](platforms-and-profiles.md): share one repo across different machines.
- [Symlinks](symlinks.md): put repo files where programs expect them.
- [Packages](packages.md): install packages with `pacman`, `paru`, `apt`, or Homebrew.
- [Services](services.md): start or enable systemd and Homebrew services.
- [Fonts](fonts.md): keep fonts in the repo and install them for the current OS.
- [User](user.md): set the login shell or add the current user to groups.
- [State](state.md): inspect what `dots` owns, or stop tracking something.
- [Release](release.md): publish a tagged GitHub release.
