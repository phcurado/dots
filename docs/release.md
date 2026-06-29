# Release

Releases are built from tags.

```sh
git tag v0.1.0
git push origin v0.1.0
```

The release workflow builds archives for:

- `linux_amd64`
- `linux_arm64`
- `darwin_amd64`
- `darwin_arm64`

It uploads the archives and `checksums.txt` to the GitHub release. There is no
Cargo publish step.
