# SSH

## Keypairs

Declare an ED25519 keypair with an explicit path and passphrase policy:

```lua
local key = dots.ssh.keypair("personal", {
  path = "~/.ssh/id_ed25519",
  comment = "me@example.com",
  passphrase = "prompt",
})
```

Use `passphrase = false` to generate an unencrypted key. With `"prompt"`, `dots apply` lets `ssh-keygen` prompt without storing the passphrase in configuration or state.

Dots generates a keypair only when both files are absent. An existing matching keypair is adopted. If one half is missing or the two files do not match, dots reports a conflict and does not overwrite either file.

Dots enforces these permissions:

- key directory: `0700`
- private key: `0600`
- public key: `0644`

Removing the declaration stops tracking the keypair without deleting it.

## Public-key output

A keypair publishes its public key and fingerprint:

```lua
local key = dots.ssh.keypair("personal", {
  path = "~/.ssh/id_ed25519",
  passphrase = false,
})

dots.output("ssh_public_key", {
  value = key.public_key,
})

dots.output("ssh_fingerprint", {
  value = key.fingerprint,
})
```

After applying:

```sh
dots output ssh_public_key
```
