# CHANGELOG

## 0.9.0

### Changed

- `dots check` no longer writes local state; matching resources are recorded by `dots apply`.
- Missing command dependencies now fail during planning.
- Managed fonts and systemd units are protected from removal when changed outside dots.

## 0.8.2

### Fixed

- Homebrew formulae and casks are removed before their taps, preventing `brew
  untap` from failing while managed packages are still installed.

## 0.8.1

### Added

- Declarative Homebrew trust for individual formulae with
  `dots.brew.trust.formula(...)` and whole taps with `dots.brew.trust.tap(...)`.

## 0.8.0

`0.8.0` adds managed files, outputs and SSH keypairs.

### Added

- `dots.file(...)` for atomically managing regular files and Unix modes from project sources.
- `dots.output(...)` and `dots output` for publishing typed literal and resource values.
- Output changes in `dots check`, including values that become known after apply.
- `dots.ssh.keypair(...)` for generating or adopting ED25519 keypairs with explicit passphrase policies.
- Public-key and fingerprint attributes from SSH keypairs for use as outputs.

### Changed

- Removing a file or SSH keypair declaration forgets it from state without deleting the files.
- SSH keypairs enforce `0700` on the parent directory, `0600` on the private key, and `0644` on the public key.

## 0.7.1

### Fixed

- Systemd service discovery now includes timers and other unit types.
- Active timers are correctly detected instead of repeatedly appearing as pending start and enable operations.

## 0.7.0

`0.7.0` adds declarative Docker Compose applications and managed systemd unit files.

### Added

- `dots.docker.compose(...)` for applying, checking, and removing tracked Docker Compose applications.
- `dots.systemd.install(...)` for installing and removing system-wide systemd unit files.
- Automatic ordering so managed systemd units are installed before they are enabled or started, and removed after they are stopped and disabled.

### Changed

- Docker Compose projects and managed systemd unit files are persisted in local state, so removing their declarations applies the corresponding cleanup.

## 0.6.1

`0.6.1` polishes apply and state handling after the 0.6.0 release.

### Fixed

- `dots apply` now persists state for completed steps even when a later step
  fails.
- Corrupt local state errors now include a reset hint.
- Symlink conflicts now distinguish different file contents from non-file
  targets.
- Font source resolution uses clearer internal naming.

## 0.6.0

`0.6.0` expands built-in providers and makes declared symlink imports part of the normal `dots check` / `dots apply` flow.

### Added

- Package providers: `dnf`, `zypper`, `apk`, `flatpak`, and `snap`.
- Service provider: `openrc`, exposed as `dots.openrc.start(...)` and
  `dots.openrc.enable(...)`.
- Platform families `fedora` (Fedora, RHEL, CentOS, Rocky, Alma) and `suse`
  (openSUSE, SLES).

### Changed

- Explicit file imports shown by `dots check` are now applied by `dots apply`.

## 0.5.1

`0.5.1` improves diagnostics for symlink declarations whose source file is
missing.

### Fixed

- Missing symlink sources now report when the target already points to another
  source, which makes scratch-repo testing easier to understand.

## 0.5.0

`0.5.0` adds symlink-focused workflows for importing existing files into a
dotfiles repo and applying symlink-only changes.

### Added

- `dots symlink` for reviewing symlink-only changes.
- `dots symlink apply` for applying only symlink changes.

### Changed

- `dots check` now suggests `dots symlink` when an explicit symlink declaration
  can import an existing target file into the repo.
- Symlink docs and quick start now explain the import/link flow.

## 0.4.0

`0.4.0` makes providers more extensible and improves non-interactive and safety
behavior.

### Added

- `dots apply --auto-approve` for non-interactive bootstrap scripts.
- Package provider metadata for capabilities, bulk list commands, match modes,
  and package-provided capabilities.
- Service provider metadata for availability checks and bulk started/enabled
  status lists.
- Shared package list command output caching, so providers like `brew` and
  `brew-cask` can reuse one slow probe.

### Changed

- Built-in package and service provider details now live in Lua provider specs
  instead of hardcoded Rust branches.
- `dots check` provider probes are documented as state-syncing checks that may
  run configured shell commands.
- Tests now use temporary directories that clean themselves up.

### Fixed

- State loading now reports read errors instead of treating them as empty state.
- Symlink apply re-checks target safety and replaces links atomically.
- Service actions in state are enum-backed instead of re-parsed from arbitrary
  strings.

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
