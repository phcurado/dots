# Services

Some tools need a service to be running after the package is installed. Docker is
a good example on Linux. SketchyBar and Borders are good examples on macOS when
they are managed by Homebrew services.

Declare those services next to the rest of the machine setup:

```lua
if dots.platform.family == "arch" then
  dots.systemd.enable({ "docker.service" })
  dots.systemd.start({ "docker.service" })
end

if dots.platform.family == "darwin" then
  dots.brew.service.start({ "sketchybar", "borders" })
end
```

`enable` and `start` are separate. On Linux, `enable` controls whether the unit
starts at boot. `start` controls whether it is running now.

## systemd

Use `enable` when the unit should start on boot:

```lua
dots.systemd.enable({ "docker.service" })
```

Use `start` when the unit should be active now:

```lua
dots.systemd.start({ "docker.service" })
```

If you remove those declarations later, `dots check` shows the reverse operation:

- `enable` becomes `disable`
- `start` becomes `stop`

## Homebrew services

Homebrew services currently support `start`:

```lua
dots.brew.service.start({ "sketchybar", "borders" })
```

If you remove the declaration, `dots` runs `brew services stop` for that service.
