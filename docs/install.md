# Install

Use the install script to download the latest release binary:

```sh
curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

The script picks the archive for the current OS and CPU architecture, verifies
the SHA-256 checksum, and installs `dots` here:

```text
~/.local/bin/dots
```

Make sure `~/.local/bin` is in `PATH`.

## Specific version

Set `VERSION` to install a specific release:

```sh
VERSION=v0.1.0 curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

The `v` prefix is optional.

## Different install directory

Set `BIN_DIR` if the binary should be installed somewhere else:

```sh
BIN_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

## From source

From a checkout of the `dots` source repo:

```sh
make install
```

This builds the release binary and copies it to `~/.local/bin/dots`, unless
`BIN_DIR` is set.
