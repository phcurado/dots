use std::fs;
use std::process::Command;
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
fn plan_prints_symlink_and_package_changes() {
    let root = temp_dir("cli-plan");
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
        .arg("plan")
        .current_dir(&root)
        .env("HOME", &home)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Initializing state: .dots/state.json"));
    assert!(stdout.contains("+ symlink ~/.zshrc -> .zshrc"));
    assert!(stdout.contains("+ fake bat"));
    assert!(stdout.contains("Plan: 2 to create, 0 to update, 0 to destroy."));
}
