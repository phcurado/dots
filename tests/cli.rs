use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dots-{name}-{}-{id}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
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
    assert!(stdout.contains("Initializing state: .dots/state.json"));
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
