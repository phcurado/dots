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
  *" down "*) rm -f "$FAKE_DOCKER_STATE" ;;
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
