//! Parity tests for filesystem helper behavior.

use std::fs;
use std::path::PathBuf;

use pkg_rust::plus_x;

#[cfg(unix)]
#[test]
fn plus_x_adds_execute_bits_without_clearing_existing_mode()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;

    let path = temp_file("mode");
    fs::write(&path, b"demo")?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640))?;

    plus_x(&path)?;

    let mode = fs::metadata(&path)?.permissions().mode() & 0o777;
    assert_eq!(mode, 0o751);
    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn plus_x_errors_for_missing_files() {
    let path = temp_file("missing");
    let error = plus_x(&path).err();

    assert!(error.is_some());
}

fn temp_file(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("pkg-rust-plus-x-{label}-{}", std::process::id()))
}
