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
