# Install

The install script downloads a release binary from GitHub:

```sh
curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

It detects the current OS and CPU architecture, downloads the matching archive,
checks the SHA-256 checksum, and installs the binary.

By default, the binary is installed here:

```text
~/.local/bin/dots
```

Make sure that directory is in `PATH`.

## Install a specific version

Set `VERSION` if you want a specific release:

```sh
VERSION=v0.1.0 curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

The `v` prefix is optional.

## Change the install directory

Set `BIN_DIR` to install somewhere else:

```sh
BIN_DIR=/usr/local/bin sh install.sh
```

## Install from source

If you have a local checkout of this repository, run:

```sh
make install
```

This builds the release binary and copies it to `~/.local/bin/dots`, unless
`BIN_DIR` is set.
