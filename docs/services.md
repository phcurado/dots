# Services

Services describe things that should be enabled or running.

```lua
if dots.platform.family == "arch" then
  dots.systemd.enable({ "docker.service" })
  dots.systemd.start({ "docker.service" })
elseif dots.os == "macos" then
  dots.brew.service.start({ "sketchybar", "borders" })
end
```

`enable` and `start` are separate on purpose. On Linux, `enable` controls boot
startup and `start` controls the current session.

## systemd

```lua
dots.systemd.enable({ "docker.service" })
dots.systemd.start({ "docker.service" })
```

if you remove the declaration above, the following actions will be planned/applied:

- `enable` becomes `disable`
- `start` becomes `stop`

## Homebrew services

```lua
dots.brew.service.start({ "sketchybar", "borders" })
```

Homebrew services currently support `start`. Removing the declaration runs
`brew services stop`.
