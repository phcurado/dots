use std::fs;
use std::io::Write;
use std::ops::Deref;
use std::path::Path;
use std::process::{Command, Stdio};

struct TempDir(tempfile::TempDir);

impl Deref for TempDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.0.path()
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        self.0.path()
    }
}

fn temp_dir(name: &str) -> TempDir {
    TempDir(
        tempfile::Builder::new()
            .prefix(&format!("dots-{name}-"))
            .tempdir()
            .unwrap(),
    )
}

#[test]
fn check_prints_symlink_and_package_changes() {
    let root = temp_dir("cli-check");
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(root.join(".zshrc"), "").unwrap();
    fs::write(
        root.join("dots.lua"),
        r#"
        dots.symlink("~/.zshrc", ".zshrc")

        dots.provider.package("fake", {
          available = "exit 0",
          installed = "exit 1",
          install = "exit 0",
          remove = "exit 0",
        })

        dots.fake.install({ "bat" })
        "#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Created local state: .dots/state.json"));
    assert!(stdout.contains("+ symlink ~/.zshrc -> .zshrc"));
    assert!(stdout.contains("+ fake bat"));
    assert!(stdout.contains("Check: 2 to create, 0 to update, 0 to destroy."));
}

#[test]
fn symlink_imports_existing_target_file() {
    let root = temp_dir("cli-symlink");
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join(".zshrc"), "export EDITOR=nvim\n").unwrap();
    fs::write(
        root.join("dots.lua"),
        r#"dots.symlink("~/.zshrc", ".zshrc")"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Symlinks:"));
    assert!(stdout.contains("+ import ~/.zshrc -> .zshrc"));
    assert!(stdout.contains("Check: 1 to import, 0 to create, 0 to update, 0 to destroy."));
    assert!(stdout.contains("Run `dots apply` to apply these changes."));

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("apply")
        .arg("--auto-approve")
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(root.join(".zshrc")).unwrap(),
        "export EDITOR=nvim\n"
    );
    assert!(
        fs::symlink_metadata(home.join(".zshrc"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_to_string(home.join(".zshrc")).unwrap(),
        "export EDITOR=nvim\n"
    );
}

#[test]
fn default_command_checks() {
    let root = temp_dir("cli-default-check");
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(root.join(".zshrc"), "").unwrap();
    fs::write(
        root.join("dots.lua"),
        r#"dots.symlink("~/.zshrc", ".zshrc")"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("+ symlink ~/.zshrc -> .zshrc"));
}

#[test]
fn init_creates_config_and_gitignore() {
    let root = temp_dir("cli-init");

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("init")
        .current_dir(&root)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(root.join("dots.lua").exists());
    assert!(
        fs::read_to_string(root.join(".gitignore"))
            .unwrap()
            .contains(".dots/")
    );
}

#[test]
fn init_reports_when_already_initialized() {
    let root = temp_dir("cli-init-existing");
    fs::write(root.join("dots.lua"), "").unwrap();
    fs::write(root.join(".gitignore"), ".dots/\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("init")
        .current_dir(&root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Already initialized."));
}

#[test]
fn default_command_without_config_fails_without_prompt_when_not_interactive() {
    let root = temp_dir("cli-default-init");

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .current_dir(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(!root.join("dots.lua").exists());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("No dots project found."));
    assert!(stderr.contains("Run `dots init` to start."));
}

#[test]
fn check_prints_command_changes() {
    let root = temp_dir("cli-command");
    fs::write(
        root.join("dots.lua"),
        r#"
        dots.command("oh-my-zsh", {
          check = "exit 1",
          apply = "exit 0",
        })
        "#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Commands:"));
    assert!(stdout.contains("+ oh-my-zsh"));
}

#[test]
fn docker_compose_is_applied_and_removed_when_undeclared() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("cli-compose");
    let bin = root.join("bin");
    let running = root.join("running");
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        root.join("compose.yaml"),
        "services:\n  web:\n    image: nginx\n",
    )
    .unwrap();
    fs::write(
        root.join("dots.lua"),
        r#"dots.docker.compose("my-service", { file = "compose.yaml" })"#,
    )
    .unwrap();
    let docker = bin.join("docker");
    fs::write(
        &docker,
        r#"#!/bin/sh
case " $* " in
  *" compose version "*) exit 0 ;;
  *" config --services "*) printf 'web\n' ;;
  *" config "*) printf 'services:\n  web:\n    image: nginx\n' ;;
  *" ps "*)
    if [ -f "$FAKE_DOCKER_STATE" ]; then
      printf '%s\n' '{"Service":"web","State":"running","Health":""}'
    fi
    ;;
  *" up "*) touch "$FAKE_DOCKER_STATE" ;;
  *" down "*) cat >/dev/null; rm -f "$FAKE_DOCKER_STATE" ;;
  *) exit 1 ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&docker, fs::Permissions::from_mode(0o755)).unwrap();

    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["apply", "--auto-approve"])
        .current_dir(&root)
        .env("PATH", &path)
        .env("FAKE_DOCKER_STATE", &running)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(running.exists());
    assert!(
        fs::read_to_string(root.join(".dots/state.json"))
            .unwrap()
            .contains("compose:my-service")
    );

    fs::write(root.join("dots.lua"), "").unwrap();
    fs::remove_file(root.join("compose.yaml")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["apply", "--auto-approve"])
        .current_dir(&root)
        .env("PATH", &path)
        .env("FAKE_DOCKER_STATE", &running)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!running.exists());
    assert!(
        !fs::read_to_string(root.join(".dots/state.json"))
            .unwrap()
            .contains("compose:my-service")
    );
}

#[test]
fn systemd_service_is_installed_tracked_and_removed_after_source_deletion() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("cli-systemd");
    let bin = root.join("bin");
    let units = root.join("units");
    let enabled = root.join("enabled");
    let active = root.join("active");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&units).unwrap();
    fs::create_dir_all(root.join("services")).unwrap();
    fs::write(
        root.join("services/my-service.service"),
        "[Service]\nExecStart=/usr/bin/true\n",
    )
    .unwrap();
    fs::write(
        root.join("dots.lua"),
        r#"
        dots.systemd.install({ "services/my-service.service" })
        dots.systemd.enable({ "my-service.service" })
        dots.systemd.start({ "my-service.service" })
        "#,
    )
    .unwrap();

    let sudo = bin.join("sudo");
    fs::write(
        &sudo,
        r#"#!/bin/sh
if [ "$1" = "-v" ]; then exit 0; fi
exec "$@"
"#,
    )
    .unwrap();
    fs::set_permissions(&sudo, fs::Permissions::from_mode(0o755)).unwrap();
    let systemctl = bin.join("systemctl");
    fs::write(
        &systemctl,
        r#"#!/bin/sh
case "$1" in
  --version|daemon-reload) exit 0 ;;
  is-enabled) test -f "$FAKE_SYSTEMD_ENABLED" ;;
  is-active) test -f "$FAKE_SYSTEMD_ACTIVE" ;;
  enable) test -f "$DOTS_SYSTEMD_UNIT_DIR/my-service.service" && touch "$FAKE_SYSTEMD_ENABLED" ;;
  start) test -f "$DOTS_SYSTEMD_UNIT_DIR/my-service.service" && touch "$FAKE_SYSTEMD_ACTIVE" ;;
  disable) test -f "$DOTS_SYSTEMD_UNIT_DIR/my-service.service" && rm -f "$FAKE_SYSTEMD_ENABLED" ;;
  stop) test -f "$DOTS_SYSTEMD_UNIT_DIR/my-service.service" && rm -f "$FAKE_SYSTEMD_ACTIVE" ;;
  *) exit 1 ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&systemctl, fs::Permissions::from_mode(0o755)).unwrap();

    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["apply", "--auto-approve"])
        .current_dir(&root)
        .env("PATH", &path)
        .env("DOTS_SYSTEMD_UNIT_DIR", &units)
        .env("FAKE_SYSTEMD_ENABLED", &enabled)
        .env("FAKE_SYSTEMD_ACTIVE", &active)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(units.join("my-service.service")).unwrap(),
        "[Service]\nExecStart=/usr/bin/true\n"
    );
    assert!(enabled.exists());
    assert!(active.exists());
    assert!(
        fs::read_to_string(root.join(".dots/state.json"))
            .unwrap()
            .contains("systemd-unit:my-service.service")
    );

    fs::write(root.join("dots.lua"), "").unwrap();
    fs::remove_file(root.join("services/my-service.service")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["apply", "--auto-approve"])
        .current_dir(&root)
        .env("PATH", &path)
        .env("DOTS_SYSTEMD_UNIT_DIR", &units)
        .env("FAKE_SYSTEMD_ENABLED", &enabled)
        .env("FAKE_SYSTEMD_ACTIVE", &active)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!units.join("my-service.service").exists());
    assert!(!enabled.exists());
    assert!(!active.exists());
    assert!(
        !fs::read_to_string(root.join(".dots/state.json"))
            .unwrap()
            .contains("systemd-unit:my-service.service")
    );
}

#[test]
fn check_prints_capability_conflicts() {
    let root = temp_dir("cli-capability");
    fs::write(
        root.join("dots.lua"),
        r#"
        dots.provider.package("fake", {
          available = "exit 1",
          installed = "exit 1",
          install = "exit 0",
          remove = "exit 0",
        })

        dots.fake.install({ "bat" })
        "#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Capabilities:"));
    assert!(stdout.contains("! fake is not available"));
}

#[test]
fn apply_auto_approve_skips_confirmation() {
    let root = temp_dir("cli-apply-auto-approve");
    fs::write(
        root.join("dots.lua"),
        r#"
        dots.provider.package("fake", {
          available = "exit 0",
          installed = "exit 1",
          install = "exit 0",
          remove = "exit 0",
        })

        dots.fake.install({ "bat" })
        "#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("apply")
        .arg("--auto-approve")
        .current_dir(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("Type 'yes' to apply these changes."));
    assert!(stdout.contains("Apply complete:"));
}

#[test]
fn apply_requires_yes() {
    let root = temp_dir("cli-apply-confirm");
    fs::write(
        root.join("dots.lua"),
        r#"
        dots.provider.package("fake", {
          available = "exit 0",
          installed = "exit 1",
          install = "exit 0",
          remove = "exit 0",
        })

        dots.fake.install({ "bat" })
        "#,
    )
    .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("apply")
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child.stdin.as_mut().unwrap().write_all(b"no\n").unwrap();

    let output = child.wait_with_output().unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stdout.contains("Type 'yes' to apply these changes."));
    assert!(stderr.contains("apply cancelled"));
}

#[test]
fn outputs_are_stored_and_read_as_typed_values() {
    let root = temp_dir("cli-outputs");
    fs::write(
        root.join("dots.lua"),
        r#"
        dots.output("machine_name", { value = "workstation" })
        dots.output("ports", { value = { 80, 443 } })
        dots.output("settings", { value = { enabled = true, retries = 3 } })
        "#,
    )
    .unwrap();

    let check = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(check.status.success());
    let stdout = String::from_utf8(check.stdout).unwrap();
    assert!(stdout.contains("Outputs:"));
    assert!(stdout.contains("+ machine_name = \"workstation\""));

    let apply = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["apply", "--auto-approve"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(apply.status.success());

    let single = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["output", "machine_name"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(single.status.success());
    assert_eq!(String::from_utf8(single.stdout).unwrap(), "workstation\n");

    let list = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("output")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8(list.stdout).unwrap();
    assert!(stdout.contains("ports = [80,443]"));
    assert!(stdout.contains("settings = {\"enabled\":true,\"retries\":3}"));
}

#[test]
fn managed_file_is_created_updated_and_forgotten_without_deletion() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("cli-managed-file");
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(root.join("files")).unwrap();
    fs::write(root.join("files/ssh-config"), "Host github.com\n").unwrap();
    fs::write(
        root.join("dots.lua"),
        r#"dots.file("~/.ssh/config", { source = "files/ssh-config", mode = "0600" })"#,
    )
    .unwrap();

    let check = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(check.status.success());
    assert!(
        String::from_utf8(check.stdout)
            .unwrap()
            .contains("+ ~/.ssh/config")
    );

    let apply = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["apply", "--auto-approve"])
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&apply.stdout),
        String::from_utf8_lossy(&apply.stderr)
    );
    let target = home.join(".ssh/config");
    assert_eq!(fs::read_to_string(&target).unwrap(), "Host github.com\n");
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o600
    );

    fs::write(root.join("files/ssh-config"), "Host gitlab.com\n").unwrap();
    let update = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["apply", "--auto-approve"])
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(update.status.success());
    assert_eq!(fs::read_to_string(&target).unwrap(), "Host gitlab.com\n");

    fs::write(root.join("dots.lua"), "").unwrap();
    let forget = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["apply", "--auto-approve"])
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(forget.status.success());
    assert!(target.exists());
    assert!(
        String::from_utf8(forget.stdout)
            .unwrap()
            .contains("1 forgotten")
    );
}

#[test]
fn managed_file_adopts_identical_target_and_conflicts_with_different_target() {
    let root = temp_dir("cli-managed-file-adoption");
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(root.join("source"), "same\n").unwrap();
    fs::write(home.join("target"), "same\n").unwrap();
    fs::write(
        root.join("dots.lua"),
        r#"dots.file("~/target", { source = "source" })"#,
    )
    .unwrap();

    let adopted = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(adopted.status.success());
    assert!(
        String::from_utf8(adopted.stdout)
            .unwrap()
            .contains("No changes.")
    );

    fs::write(home.join("target"), "different\n").unwrap();
    fs::remove_dir_all(root.join(".dots")).unwrap();
    let conflict = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(conflict.status.success());
    let stdout = String::from_utf8(conflict.stdout).unwrap();
    assert!(stdout.contains("Files:"));
    assert!(stdout.contains("target exists but is not managed"));
}

#[test]
fn ssh_keypair_generates_publishes_output_and_is_forgotten_without_deletion() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("cli-ssh-keypair");
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        root.join("dots.lua"),
        r#"
        local key = dots.ssh.keypair("personal", {
          path = "~/.ssh/id_ed25519",
          comment = "test@example.com",
          passphrase = false,
        })
        dots.output("ssh_public_key", { value = key.public_key })
        "#,
    )
    .unwrap();

    let check = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(check.status.success());
    let stdout = String::from_utf8(check.stdout).unwrap();
    assert!(stdout.contains("SSH keypairs:"));
    assert!(stdout.contains("+ personal ~/.ssh/id_ed25519"));

    let apply = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["apply", "--auto-approve"])
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let private = home.join(".ssh/id_ed25519");
    let public = home.join(".ssh/id_ed25519.pub");
    assert!(private.exists());
    assert!(public.exists());
    assert_eq!(
        fs::metadata(&private).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&public).unwrap().permissions().mode() & 0o777,
        0o644
    );
    assert_eq!(
        fs::metadata(home.join(".ssh"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["output", "ssh_public_key"])
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("ssh-ed25519 ")
    );

    fs::write(root.join("dots.lua"), "").unwrap();
    let forget = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["apply", "--auto-approve"])
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(forget.status.success());
    assert!(private.exists());
    assert!(public.exists());
}

#[test]
fn ssh_keypair_conflicts_when_one_half_is_missing() {
    let root = temp_dir("cli-ssh-keypair-half");
    let home = root.join("home");
    fs::create_dir_all(home.join(".ssh")).unwrap();
    fs::write(home.join(".ssh/id_ed25519.pub"), "ssh-ed25519 invalid\n").unwrap();
    fs::write(
        root.join("dots.lua"),
        r#"dots.ssh.keypair("personal", { path = "~/.ssh/id_ed25519", passphrase = false })"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("private key is missing")
    );
}

#[test]
fn ssh_keypair_fixes_permissions_on_an_existing_pair() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("cli-ssh-keypair-permissions");
    let home = root.join("home");
    let ssh = home.join(".ssh");
    fs::create_dir_all(&ssh).unwrap();
    let private = ssh.join("id_ed25519");
    let generated = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(&private)
        .status()
        .unwrap();
    assert!(generated.success());
    fs::set_permissions(&private, fs::Permissions::from_mode(0o644)).unwrap();
    fs::write(
        root.join("dots.lua"),
        r#"dots.ssh.keypair("personal", { path = "~/.ssh/id_ed25519", passphrase = false })"#,
    )
    .unwrap();

    let check = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(check.status.success());
    let stdout = String::from_utf8(check.stdout).unwrap();
    assert!(stdout.contains("(permissions)"), "{stdout}");

    let apply = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["apply", "--auto-approve"])
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(apply.status.success());
    assert_eq!(
        fs::metadata(private).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn encrypted_ssh_keypair_conflicts_with_false_without_prompting() {
    let root = temp_dir("cli-ssh-keypair-encrypted-policy");
    let home = root.join("home");
    let ssh = home.join(".ssh");
    fs::create_dir_all(&ssh).unwrap();
    let private = ssh.join("id_ed25519");
    let generated = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "test-passphrase", "-f"])
        .arg(&private)
        .status()
        .unwrap();
    assert!(generated.success());
    fs::write(
        root.join("dots.lua"),
        r#"dots.ssh.keypair("personal", { path = "~/.ssh/id_ed25519", passphrase = false })"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .env("HOME", &home)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("private key is encrypted or invalid, but passphrase is false")
    );
}

#[test]
fn ssh_keypair_reports_mismatched_public_key_as_a_conflict() {
    let root = temp_dir("cli-ssh-keypair-mismatch");
    let home = root.join("home");
    let ssh = home.join(".ssh");
    fs::create_dir_all(&ssh).unwrap();
    let private = ssh.join("id_ed25519");
    let other = ssh.join("other");
    assert!(
        Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(&private)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(&other)
            .status()
            .unwrap()
            .success()
    );
    fs::copy(other.with_extension("pub"), private.with_extension("pub")).unwrap();
    fs::write(
        root.join("dots.lua"),
        r#"dots.ssh.keypair("personal", { path = "~/.ssh/id_ed25519", passphrase = false })"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("public key does not match private key")
    );
}

#[test]
fn output_changes_are_planned_and_only_persisted_by_apply() {
    let root = temp_dir("cli-output-plan");
    fs::write(
        root.join("dots.lua"),
        r#"dots.output("example", { value = "old" })"#,
    )
    .unwrap();
    assert!(
        Command::new(env!("CARGO_BIN_EXE_dots"))
            .args(["apply", "--auto-approve"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );

    fs::write(
        root.join("dots.lua"),
        r#"dots.output("example", { value = "new" })"#,
    )
    .unwrap();
    let check = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(check.status.success());
    let stdout = String::from_utf8(check.stdout).unwrap();
    assert!(stdout.contains("~ example: \"old\" → \"new\""));

    let stale = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["output", "example"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(stale.status.success());
    assert_eq!(String::from_utf8(stale.stdout).unwrap(), "old\n");

    let apply = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["apply", "--auto-approve"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(apply.status.success());
    let current = Command::new(env!("CARGO_BIN_EXE_dots"))
        .args(["output", "example"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(current.stdout).unwrap(), "new\n");

    fs::write(root.join("dots.lua"), "").unwrap();
    let removal = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(removal.status.success());
    assert!(
        String::from_utf8(removal.stdout)
            .unwrap()
            .contains("- example = \"new\"")
    );
}

#[test]
fn ssh_keypair_true_plans_prompted_generation_without_prompting_during_check() {
    let root = temp_dir("cli-ssh-keypair-passphrase");
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        root.join("dots.lua"),
        r#"dots.ssh.keypair("personal", { path = "~/.ssh/id_ed25519", passphrase = true })"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_dots"))
        .arg("check")
        .current_dir(&root)
        .env("HOME", &home)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("+ personal ~/.ssh/id_ed25519")
    );
}
