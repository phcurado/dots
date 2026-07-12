use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::PathBuf;
use std::process::{self, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};

use crate::managed_file::{digest_file, ensure_mode, write_file_atomically};
use crate::state::{State, StateResource};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PassphrasePolicy {
    None,
    Prompt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SshKeypairResource {
    pub(crate) name: String,
    pub(crate) private_path: PathBuf,
    pub(crate) comment: Option<String>,
    pub(crate) passphrase: PassphrasePolicy,
}

#[derive(Debug, Clone)]
pub(crate) struct KeypairObservation {
    pub(crate) public_key: String,
    pub(crate) fingerprint: String,
    pub(crate) private_digest: String,
    pub(crate) public_digest: String,
    pub(crate) encrypted: bool,
}

pub(crate) enum KeypairStatus {
    Missing,
    Current(KeypairObservation),
    PermissionDrift(KeypairObservation),
    ValidationRequired,
    Conflict(String),
}

impl SshKeypairResource {
    pub(crate) fn public_path(&self) -> PathBuf {
        PathBuf::from(format!("{}.pub", self.private_path.display()))
    }
}

pub(crate) fn keypair_id_for(resource: &SshKeypairResource) -> String {
    format!("ssh-keypair:{}", resource.name)
}

pub(crate) fn inspect_keypair(
    resource: &SshKeypairResource,
    state: &State,
) -> Result<KeypairStatus> {
    let private = fs::symlink_metadata(&resource.private_path);
    let public_path = resource.public_path();
    let public = fs::symlink_metadata(&public_path);
    match (&private, &public) {
        (Err(left), Err(right))
            if left.kind() == std::io::ErrorKind::NotFound
                && right.kind() == std::io::ErrorKind::NotFound =>
        {
            return Ok(KeypairStatus::Missing);
        }
        (Ok(_), Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(KeypairStatus::Conflict("public key is missing".to_string()));
        }
        (Err(error), Ok(_)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(KeypairStatus::Conflict(
                "private key is missing".to_string(),
            ));
        }
        (Err(error), _) | (_, Err(error)) => {
            bail!("failed to inspect SSH keypair: {error}")
        }
        (Ok(private), Ok(public)) if !private.is_file() || !public.is_file() => {
            return Ok(KeypairStatus::Conflict(
                "keypair paths must be regular files".to_string(),
            ));
        }
        _ => {}
    }

    let private_digest = digest_file(&resource.private_path)?;
    let public_digest = digest_file(&public_path)?;
    if let Some(StateResource::SshKeypair {
        private_digest: stored_private,
        public_digest: stored_public,
        public_key,
        fingerprint,
        encrypted,
        ..
    }) = state.resources.get(&keypair_id_for(resource))
        && *stored_private == private_digest
        && *stored_public == public_digest
    {
        if encryption_conflicts(resource.passphrase, *encrypted) {
            return Ok(KeypairStatus::Conflict(
                "existing key does not match the declared passphrase policy".to_string(),
            ));
        }
        let observation = KeypairObservation {
            public_key: public_key.clone(),
            fingerprint: fingerprint.clone(),
            private_digest,
            public_digest,
            encrypted: *encrypted,
        };
        return Ok(if permissions_match(resource)? {
            KeypairStatus::Current(observation)
        } else {
            KeypairStatus::PermissionDrift(observation)
        });
    }

    match derive_public(resource, false)? {
        Some(derived) => {
            let observation = match validate_derived(resource, derived, false) {
                Ok(observation) => observation,
                Err(error) => return Ok(KeypairStatus::Conflict(error.to_string())),
            };
            if encryption_conflicts(resource.passphrase, false) {
                return Ok(KeypairStatus::Conflict(
                    "existing key does not match the declared passphrase policy".to_string(),
                ));
            }
            Ok(if permissions_match(resource)? {
                KeypairStatus::Current(observation)
            } else {
                KeypairStatus::PermissionDrift(observation)
            })
        }
        None if resource.passphrase == PassphrasePolicy::Prompt => {
            Ok(KeypairStatus::ValidationRequired)
        }
        None => Ok(KeypairStatus::Conflict(
            "private key is encrypted or invalid, but passphrase is false".to_string(),
        )),
    }
}

pub(crate) fn generate_keypair(resource: &SshKeypairResource, state: &mut State) -> Result<()> {
    if resource.private_path.exists() || resource.public_path().exists() {
        bail!("refusing to generate over an existing keypair");
    }
    let parent = resource
        .private_path
        .parent()
        .context("SSH private key path has no parent")?;
    fs::create_dir_all(parent)?;
    ensure_mode(parent, 0o700)?;

    let temporary_dir = temporary_directory(parent)?;
    let temporary_resource = SshKeypairResource {
        private_path: temporary_dir.join("key"),
        ..resource.clone()
    };
    let result = (|| -> Result<KeypairObservation> {
        let mut command = Command::new("ssh-keygen");
        command
            .args(["-t", "ed25519", "-f"])
            .arg(&temporary_resource.private_path);
        if let Some(comment) = &resource.comment {
            command.arg("-C").arg(comment);
        }
        if resource.passphrase == PassphrasePolicy::None {
            command.args(["-N", ""]);
        }
        let status = command
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("failed to run ssh-keygen")?;
        if !status.success() {
            bail!("ssh-keygen failed");
        }

        let public_path = temporary_resource.public_path();
        let observation = match resource.passphrase {
            PassphrasePolicy::None => {
                let derived = derive_public(&temporary_resource, false)?
                    .context("failed to validate generated SSH private key")?;
                validate_derived(&temporary_resource, derived, false)?
            }
            PassphrasePolicy::Prompt => KeypairObservation {
                public_key: fs::read_to_string(&public_path)?.trim().to_string(),
                fingerprint: fingerprint(&public_path)?,
                private_digest: digest_file(&temporary_resource.private_path)?,
                public_digest: digest_file(&public_path)?,
                encrypted: true,
            },
        };
        ensure_keypair_modes(&temporary_resource)?;
        Ok(observation)
    })();

    let observation = match result {
        Ok(observation) => observation,
        Err(error) => {
            let _ = fs::remove_dir_all(&temporary_dir);
            return Err(error);
        }
    };

    let temporary_public = temporary_resource.public_path();
    fs::rename(&temporary_resource.private_path, &resource.private_path)?;
    if let Err(error) = fs::rename(&temporary_public, resource.public_path()) {
        let _ = fs::remove_file(&resource.private_path);
        let _ = fs::remove_dir_all(&temporary_dir);
        return Err(error.into());
    }
    let _ = fs::remove_dir(&temporary_dir);

    state.resources.insert(
        keypair_id_for(resource),
        state_keypair(resource, &observation),
    );
    Ok(())
}

pub(crate) fn adopt_keypair(resource: &SshKeypairResource, state: &mut State) -> Result<()> {
    let derived = derive_public(resource, true)?
        .context("failed to validate SSH private key with the provided passphrase")?;
    let encrypted = derive_public(resource, false)?.is_none();
    if encryption_conflicts(resource.passphrase, encrypted) {
        bail!("existing key does not match the declared passphrase policy");
    }
    let observation = validate_derived(resource, derived, encrypted)?;
    ensure_keypair_modes(resource)?;
    state.resources.insert(
        keypair_id_for(resource),
        state_keypair(resource, &observation),
    );
    Ok(())
}

pub(crate) fn fix_keypair_permissions(
    resource: &SshKeypairResource,
    observation: &KeypairObservation,
    state: &mut State,
) -> Result<()> {
    ensure_keypair_modes(resource)?;
    state.resources.insert(
        keypair_id_for(resource),
        state_keypair(resource, observation),
    );
    Ok(())
}

pub(crate) fn state_keypair(
    resource: &SshKeypairResource,
    observation: &KeypairObservation,
) -> StateResource {
    StateResource::SshKeypair {
        name: resource.name.clone(),
        private_path: resource.private_path.clone(),
        public_path: resource.public_path(),
        private_digest: observation.private_digest.clone(),
        public_digest: observation.public_digest.clone(),
        fingerprint: observation.fingerprint.clone(),
        public_key: observation.public_key.clone(),
        encrypted: observation.encrypted,
    }
}

fn validate_derived(
    resource: &SshKeypairResource,
    derived: String,
    encrypted: bool,
) -> Result<KeypairObservation> {
    let public_path = resource.public_path();
    let public_key = fs::read_to_string(&public_path)
        .with_context(|| format!("failed to read {}", public_path.display()))?
        .trim()
        .to_string();
    if key_material(&derived) != key_material(&public_key) {
        bail!("public key does not match private key");
    }
    Ok(KeypairObservation {
        fingerprint: fingerprint(&public_path)?,
        private_digest: digest_file(&resource.private_path)?,
        public_digest: digest_file(&public_path)?,
        public_key,
        encrypted,
    })
}

fn derive_public(resource: &SshKeypairResource, prompt: bool) -> Result<Option<String>> {
    let temporary = if mode(&resource.private_path)? & 0o077 != 0 {
        let parent = resource
            .private_path
            .parent()
            .context("SSH private key path has no parent")?;
        let directory = temporary_directory(parent)?;
        let path = directory.join("key");
        write_file_atomically(&path, &fs::read(&resource.private_path)?, Some(0o600))?;
        Some((directory, path))
    } else {
        None
    };
    let private_path = temporary
        .as_ref()
        .map(|(_, path)| path)
        .unwrap_or(&resource.private_path);

    let mut command = Command::new("ssh-keygen");
    command.arg("-y").arg("-f").arg(private_path);
    if !prompt {
        command.args(["-P", ""]);
    }
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(if prompt {
            Stdio::inherit()
        } else {
            Stdio::null()
        });
    let output = command.output().context("failed to run ssh-keygen");
    if let Some((directory, _)) = temporary {
        let _ = fs::remove_dir_all(directory);
    }
    let output = output?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8(output.stdout)?.trim().to_string()))
}

fn fingerprint(public_path: &PathBuf) -> Result<String> {
    let output = Command::new("ssh-keygen")
        .args(["-lf"])
        .arg(public_path)
        .args(["-E", "sha256"])
        .output()?;
    if !output.status.success() {
        bail!("failed to fingerprint SSH public key");
    }
    String::from_utf8(output.stdout)?
        .split_whitespace()
        .nth(1)
        .map(str::to_string)
        .context("ssh-keygen returned an invalid fingerprint")
}

fn key_material(public_key: &str) -> Option<(&str, &str)> {
    let mut fields = public_key.split_whitespace();
    Some((fields.next()?, fields.next()?))
}

fn encryption_conflicts(policy: PassphrasePolicy, encrypted: bool) -> bool {
    encrypted != (policy == PassphrasePolicy::Prompt)
}

fn permissions_match(resource: &SshKeypairResource) -> Result<bool> {
    let parent = resource
        .private_path
        .parent()
        .context("SSH private key path has no parent")?;
    Ok(mode(parent)? == 0o700
        && mode(&resource.private_path)? == 0o600
        && mode(&resource.public_path())? == 0o644)
}

fn ensure_keypair_modes(resource: &SshKeypairResource) -> Result<()> {
    let parent = resource
        .private_path
        .parent()
        .context("SSH private key path has no parent")?;
    ensure_mode(parent, 0o700)?;
    ensure_mode(&resource.private_path, 0o600)?;
    ensure_mode(&resource.public_path(), 0o644)
}

fn temporary_directory(parent: &std::path::Path) -> Result<PathBuf> {
    for _ in 0..100 {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".dots-ssh-{}-{counter}.tmp", process::id()));
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    bail!("could not create temporary SSH key directory")
}

fn mode(path: &std::path::Path) -> Result<u32> {
    Ok(fs::metadata(path)?.permissions().mode() & 0o7777)
}
