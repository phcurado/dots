# Install

Install the latest GitHub release:

```sh
curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

Install a specific version:

```sh
VERSION=v0.1.0 curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

Install from source:

```sh
make install
```

By default the binary is installed to `~/.local/bin/dots`. Override the target
directory with `BIN_DIR`:

```sh
BIN_DIR=/usr/local/bin sh install.sh
```
