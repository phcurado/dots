# Outputs

Outputs publish resource values from a dots configuration.

```lua
dots.output("machine_name", {
  value = "workstation",
})

dots.output("ports", {
  value = { 80, 443 },
})

dots.output("settings", {
  value = {
    enabled = true,
    retries = 3,
  },
})
```

Output values may be strings, numbers, booleans, arrays or objects.

## Read outputs

List outputs:

```sh
dots output
```

Read one value:

```sh
dots output machine_name
```

Run `dots check` or `dots apply` after changing output declarations to refresh their stored values.
