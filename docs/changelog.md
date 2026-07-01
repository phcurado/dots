# Changelog

## 0.3.1

`0.3.1` smooths out first-run behavior and fixes state refresh for resources
that already match the machine.

### Added

- `yay` package provider and `dots.yay.enable(...)`.
- Broader starter config from `dots init`, including Arch, Debian/Ubuntu, macOS,
  services, fonts, and profiles.
- GitHub link in the documentation navbar.

### Changed

- `dots`, `dots check`, and `dots apply` now report a missing project and point
  to `dots init` instead of opening an init prompt.
- `dots init` has quieter output and a cleaner starter `dots.lua`.
- `dots check` now records declared packages, services, and fonts that already
  match the machine, not just symlinks and groups.
- The logo now has a transparent background.

## 0.3.0

`0.3.0` makes groups explicit and cleans up the release/docs setup.

### Added

- `dots.group.create(...)` for declaring Linux groups that should exist.
- `dots.user.add_to_groups(...)` for adding the current user to existing or declared groups.
- Removal planning for tracked groups and group memberships.
- State sync for declared existing groups and memberships during `dots check`.
- `mise.toml` for the project toolchain.
- `dprint` markdown checks in CI.

### Changed

- Removed the ambiguous `dots.user.groups(...)` API.
- Documented group creation, membership, and removal separately.
- CI now installs tools through mise.

## 0.2.0

`0.2.0` added the pieces needed to use `dots` as a real workstation manager.

### Added

- Package providers for `pacman`, `paru`, `apt`, Homebrew formulae, casks, and taps.
- Service providers for systemd and Homebrew services.
- Font installation from the repo.
- User shell management.
- Checked setup commands with `dots.command(...)`.
- Dependency ordering with `needs` and `provides`.
- Provider bootstrap helpers such as `dots.brew.enable()` and `dots.paru.enable(...)`.
- Platform facts and profiles.
- VitePress documentation and GitHub Pages workflow.

## 0.1.0

Initial release.

### Added

- `dots init`.
- `dots check` and `dots apply`.
- Interactive apply approval.
- `.dots/state.json`.
- `dots state list` and `dots state forget`.
- Conservative symlink management with relative links and stale-link cleanup.
