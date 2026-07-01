# Services

A service is a background process managed by the operating system. It might be a
database, a sync daemon, a VPN client, Docker, or another program that should
keep running without being started by hand.

On Linux, `dots` supports systemd services. On macOS, it supports services
managed by Homebrew.

```lua
if dots.platform.family == "arch" then
  dots.systemd.enable({ "docker.service" })
  dots.systemd.start({ "docker.service" })
end

if dots.platform.family == "darwin" then
  dots.brew.enable()
  dots.brew.install({ "postgresql@16" })
  dots.brew.service.start({ "postgresql@16" })
end
```

`enable` and `start` are different operations:

- `enable`: start the service automatically on boot
- `start`: start the service now

Use both when the service should run now and after reboot. Use only `start` when
the service should be active for the current profile but not necessarily enabled
at boot.

## systemd

Use `dots.systemd` for Linux system services:

```lua
dots.systemd.enable({ "docker.service" })
dots.systemd.start({ "docker.service" })
```

If those declarations are removed later, `dots check` shows the inverse action:

- `enable` becomes `disable`
- `start` becomes `stop`

## Homebrew services

Use `dots.brew.service` for services managed by Homebrew:

```lua
dots.brew.enable()
dots.brew.install({ "postgresql@16" })
dots.brew.service.start({ "postgresql@16" })
```

If the declaration is removed later, `dots` runs `brew services stop` for that
service.

## Profiles

Profiles can decide which services are active on the same machine. For example,
a work profile might start a VPN service while a personal profile leaves it
stopped:

```lua
if dots.profile == "work" then
  dots.systemd.start({ "example-vpn.service" })
end
```

Switch profiles with:

```sh
dots --profile work apply
dots --profile personal apply
```

`dots` only stops services it already manages. If a service was started manually
outside `dots`, it is left alone until it is declared and applied once.
