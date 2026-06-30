# Services

Services are resources backed by a service manager. `dots check` reports whether
the service action is missing, and `dots apply` runs the corresponding command.

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

`enable` and `start` are separate actions. `enable` means the service should
start at boot. `start` means the service should be running now.

## systemd

Use `enable` when the unit should start on boot:

```lua
dots.systemd.enable({ "docker.service" })
```

Use `start` when the unit should be active now:

```lua
dots.systemd.start({ "docker.service" })
```

If you remove those declarations later, `dots check` shows the reverse action:

- `enable` becomes `disable`
- `start` becomes `stop`

## Homebrew services

Homebrew services currently support `start`:

```lua
dots.brew.enable()
dots.brew.install({ "postgresql@16" })
dots.brew.service.start({ "postgresql@16" })
```

If you remove the declaration, `dots` runs `brew services stop` for that service.
