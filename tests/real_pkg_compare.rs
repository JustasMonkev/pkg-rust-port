#![allow(missing_docs)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const DEFAULT_REAL_TARGET: &str = "node18-macos-x64";

#[test]
fn compares_selected_fixtures_with_real_pkg_when_enabled() -> Result<(), Box<dyn std::error::Error>>
{
    if std::env::var_os("PKG_RUST_REAL_PKG_COMPARE").is_none() {
        eprintln!("skipping real pkg comparison: PKG_RUST_REAL_PKG_COMPARE is not set");
        return Ok(());
    }

    let pkg_bin = required_path("PKG_RUST_REAL_PKG_BIN")?;
    let cache = required_path("PKG_CACHE_PATH")?;
    let target = real_target();
    let workspace =
        std::env::temp_dir().join(format!("pkg-rust-real-pkg-compare-{}", std::process::id()));
    let _ignored = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace)?;

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut cases = vec![
        CompareCase {
            name: "snapshot-path",
            source: manifest.join("test/test-50-api"),
            input: "test-x-index.js",
            run_from_output_dir: false,
            prepare_output_dir: None,
        },
        CompareCase {
            name: "module-parent",
            source: manifest.join("test/test-50-module-parent"),
            input: "test-x-index.js",
            run_from_output_dir: false,
            prepare_output_dir: None,
        },
        CompareCase {
            name: "mountpoints",
            source: manifest.join("test/test-50-mountpoints"),
            input: "test-x-index.js",
            run_from_output_dir: true,
            prepare_output_dir: Some(copy_plugins_d_ext),
        },
        CompareCase {
            name: "fs-runtime-layer-2",
            source: manifest.join("test/test-50-fs-runtime-layer-2"),
            input: "test-x-index.js",
            run_from_output_dir: false,
            prepare_output_dir: None,
        },
        CompareCase {
            name: "require-edge-cases",
            source: manifest.join("test/test-50-require-edge-cases"),
            input: "test-x-index.js",
            run_from_output_dir: false,
            prepare_output_dir: None,
        },
    ];

    let readdir_source = create_readdir_fixture(&workspace)?;
    cases.push(CompareCase {
        name: "readdir-bundled-dir",
        source: readdir_source,
        input: "entry.js",
        run_from_output_dir: false,
        prepare_output_dir: None,
    });

    for case in cases {
        compare_case(&case, &workspace, &pkg_bin, &cache, &target)?;
    }

    fs::remove_dir_all(&workspace)?;
    Ok(())
}

struct CompareCase {
    name: &'static str,
    source: PathBuf,
    input: &'static str,
    run_from_output_dir: bool,
    prepare_output_dir: Option<PrepareOutputDir>,
}

struct PackageResult {
    output: PathBuf,
    run: Output,
    snapshot_strings: BTreeSet<String>,
}

type PrepareOutputDir = fn(&Path, &Path) -> Result<(), Box<dyn std::error::Error>>;

fn compare_case(
    case: &CompareCase,
    workspace: &Path,
    pkg_bin: &Path,
    cache: &Path,
    target: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = copy_fixture(case, workspace)?;
    let real = package_and_run("real", case, &fixture, workspace, pkg_bin, cache, target)?;
    let rust = package_and_run(
        "rust",
        case,
        &fixture,
        workspace,
        Path::new(env!("CARGO_BIN_EXE_pkg")),
        cache,
        target,
    )?;

    assert_eq!(
        real.run.stdout,
        rust.run.stdout,
        "{} stdout diverged\nreal:\n{}\nrust:\n{}",
        case.name,
        String::from_utf8_lossy(&real.run.stdout),
        String::from_utf8_lossy(&rust.run.stdout)
    );
    assert_eq!(
        real.run.stderr,
        rust.run.stderr,
        "{} stderr diverged\nreal:\n{}\nrust:\n{}",
        case.name,
        String::from_utf8_lossy(&real.run.stderr),
        String::from_utf8_lossy(&rust.run.stderr)
    );
    assert_eq!(
        real.snapshot_strings,
        rust.snapshot_strings,
        "{} embedded /snapshot strings diverged\nreal-only: {:?}\nrust-only: {:?}",
        case.name,
        real.snapshot_strings
            .difference(&rust.snapshot_strings)
            .collect::<Vec<_>>(),
        rust.snapshot_strings
            .difference(&real.snapshot_strings)
            .collect::<Vec<_>>()
    );

    println!(
        "{} ok: stdout={} bytes, snapshot strings={}",
        case.name,
        real.run.stdout.len(),
        real.snapshot_strings.len()
    );

    let _ignored = fs::remove_file(real.output);
    let _ignored = fs::remove_file(rust.output);
    Ok(())
}

fn package_and_run(
    label: &str,
    case: &CompareCase,
    fixture: &Path,
    workspace: &Path,
    pkg_bin: &Path,
    cache: &Path,
    target: &str,
) -> Result<PackageResult, Box<dyn std::error::Error>> {
    let output = workspace.join(format!("{}-{}", case.name, label));
    let package = Command::new(pkg_bin)
        .current_dir(fixture)
        .env("PKG_CACHE_PATH", cache)
        .args(["--public", "--no-bytecode", "--target"])
        .arg(target)
        .arg("--output")
        .arg(&output)
        .arg(case.input)
        .output()?;
    assert!(
        package.status.success(),
        "{} {} packaging failed: {}{}",
        case.name,
        label,
        String::from_utf8_lossy(&package.stdout),
        String::from_utf8_lossy(&package.stderr)
    );

    if let Some(prepare_output_dir) = case.prepare_output_dir {
        let output_dir = output
            .parent()
            .ok_or_else(|| format!("{} output path has no parent", case.name))?;
        prepare_output_dir(fixture, output_dir)?;
    }

    let run_cwd = if case.run_from_output_dir {
        output
            .parent()
            .ok_or_else(|| format!("{} output path has no parent", case.name))?
    } else {
        fixture
    };
    let run = Command::new(&output).current_dir(run_cwd).output()?;
    assert!(
        run.status.success(),
        "{} {} executable failed: {}{}",
        case.name,
        label,
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let snapshot_strings = extract_snapshot_strings(&fs::read(&output)?);
    Ok(PackageResult {
        output,
        run,
        snapshot_strings,
    })
}

fn extract_snapshot_strings(bytes: &[u8]) -> BTreeSet<String> {
    let mut strings = BTreeSet::new();
    let mut current = Vec::new();
    for byte in bytes {
        if (0x20..=0x7e).contains(byte) {
            current.push(*byte);
        } else {
            push_snapshot_string(&mut strings, &current);
            current.clear();
        }
    }
    push_snapshot_string(&mut strings, &current);
    strings
        .into_iter()
        .filter(|value| !value.contains("/snapshot/appname/node_modules/sharp"))
        .filter(|value| value.starts_with("/snapshot/"))
        .filter(|value| {
            value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(byte, b'/' | b'.' | b'-' | b'_' | b'#' | b'@')
            })
        })
        .collect()
}

fn push_snapshot_string(strings: &mut BTreeSet<String>, bytes: &[u8]) {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return;
    };
    for piece in text.split(['"', '\'', '`', '\\', ' ', '\t', '\r', '\n']) {
        if let Some(start) = piece.find("/snapshot") {
            strings.insert(piece[start..].trim_end_matches([';', ',', ')']).to_owned());
        }
    }
}

fn copy_fixture(
    case: &CompareCase,
    workspace: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let target = workspace.join(format!("{}-fixture", case.name));
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    copy_directory(&case.source, &target)?;
    Ok(target)
}

fn copy_directory(source: &Path, target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_directory(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn copy_plugins_d_ext(
    fixture_dir: &Path,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    copy_directory(
        &fixture_dir.join("plugins-D-ext"),
        &output_dir.join("plugins-D-ext"),
    )
}

fn create_readdir_fixture(workspace: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let fixture = workspace.join("readdir-source");
    fs::create_dir_all(&fixture)?;
    fs::write(
        fixture.join("entry.js"),
        "const fs = require('fs'); console.log(fs.readdirSync(__dirname).sort().join(','));\n",
    )?;
    fs::write(fixture.join("sibling.js"), "console.log('not bundled');\n")?;
    Ok(fixture)
}

fn required_path(name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let value = std::env::var_os(name).ok_or_else(|| format!("{name} is required"))?;
    let path = PathBuf::from(value);
    if !path.exists() {
        return Err(format!("{name} does not exist: {}", path.display()).into());
    }
    Ok(path)
}

fn real_target() -> String {
    std::env::var("PKG_RUST_REAL_TARGET").unwrap_or_else(|_| DEFAULT_REAL_TARGET.to_owned())
}
