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

outputs are stored into `dots` state after running `dots check` or `dots apply`.
