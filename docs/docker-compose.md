# Docker Compose

You can manage docker applications with the `dots` helpers:

```lua
dots.docker.compose("my-service", {
  file = "services/my-service/compose.yaml",
})
```

The first argument is both the dots resource name and the Docker Compose project name. The file path is resolved from the dots project root.

when applying with `dots apply`, the following command is ran under the hood:

```sh
docker compose --project-name my-service --file services/my-service/compose.yaml up --detach
```

When removing the reource, applying will run the `down` command:

```sh
docker compose --project-name my-service --file services/my-service/compose.yaml down
```

The default removal preserves volumes and images.

## Arguments

If you wish to change the behaviour above, you can override it: 

```lua
dots.docker.compose("my-service", {
  file = "services/my-service/compose.yaml",
  profiles = { "production" },
  apply = { "up", "--detach", "--wait" },
  remove = { "down", "--remove-orphans" },
})
```

## Checking state

`dots check` compares the resolved Compose configuration with the configuration recorded after the last successful apply. It also verifies that every active service has a running container and that containers with health checks are healthy.

