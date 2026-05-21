use std::collections::BTreeMap;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::error::PkgError;

const BYTECODE_FABRICATOR_SCRIPT: &str = r#"
const vm = require('vm');
const module = require('module');
let stdin = Buffer.alloc(0);

process.stdin.on('data', (chunk) => {
  stdin = Buffer.concat([stdin, chunk]);
  while (stdin.length >= 4) {
    const sizeOfSnap = stdin.readInt32LE(0);
    if (stdin.length < 4 + sizeOfSnap + 4) return;
    const sizeOfBody = stdin.readInt32LE(4 + sizeOfSnap);
    if (stdin.length < 4 + sizeOfSnap + 4 + sizeOfBody) return;

    const snap = stdin.toString('utf8', 4, 4 + sizeOfSnap);
    const startOfBody = 4 + sizeOfSnap + 4;
    const body = Buffer.alloc(sizeOfBody);
    stdin.copy(body, 0, startOfBody, startOfBody + sizeOfBody);
    stdin = stdin.subarray(startOfBody + sizeOfBody);

    const code = module.wrap(body);
    const script = new vm.Script(code, {
      filename: snap,
      produceCachedData: true,
      sourceless: true
    });

    if (!script.cachedDataProduced) {
      console.error('Pkg: Cached data not produced.');
      process.exit(2);
    }

    const header = Buffer.alloc(4);
    header.writeInt32LE(script.cachedData.length, 0);
    process.stdout.write(header);
    process.stdout.write(script.cachedData);
  }
});

process.stdin.resume();
"#;

const INERT_BAKES: &[&str] = &["--prof", "--v8-options", "--trace-opt", "--trace-deopt"];

/// Process-backed bytecode fabricator state.
#[derive(Debug, Default)]
pub struct FabricatorPool {
    children: BTreeMap<FabricatorKey, FabricatorChild>,
}

impl FabricatorPool {
    /// Build an empty fabricator pool.
    #[must_use]
    pub fn new() -> Self {
        Self {
            children: BTreeMap::new(),
        }
    }

    /// Clear any retained fabricator state.
    pub fn clear(&mut self) {
        self.children.clear();
    }

    fn fabricate(&mut self, request: FabricateRequest<'_>) -> Result<Vec<u8>, PkgError> {
        let key = FabricatorKey::from_request(request);
        if !self.children.contains_key(&key) {
            let child = FabricatorChild::spawn(&key)?;
            self.children.insert(key.clone(), child);
        }

        let result = self
            .children
            .get_mut(&key)
            .ok_or_else(|| PkgError::Pack("fabricator child was not retained".to_owned()))?
            .fabricate(request);
        if result.is_err() {
            self.children.remove(&key);
        }
        result
    }
}

/// One bytecode fabrication request.
#[derive(Clone, Copy, Debug)]
pub struct FabricateRequest<'a> {
    /// Snapshot filename passed to V8 for cached-data generation.
    pub snap: &'a str,
    /// JavaScript source bytes to compile.
    pub source: &'a [u8],
    /// Optional target Node executable used for target-specific bytecode.
    pub executable: Option<&'a Path>,
    /// Bakery flags passed to the target executable before the fabricator script.
    pub bakes: &'a [String],
}

impl<'a> FabricateRequest<'a> {
    /// Build a request using host `node` as the fabricator executable.
    #[must_use]
    pub fn new(snap: &'a str, source: &'a [u8]) -> Self {
        Self {
            snap,
            source,
            executable: None,
            bakes: &[],
        }
    }

    /// Use an explicit target Node executable for this request.
    #[must_use]
    pub fn with_executable(mut self, executable: &'a Path) -> Self {
        self.executable = Some(executable);
        self
    }

    /// Add bakery flags to the target Node invocation.
    #[must_use]
    pub fn with_bakes(mut self, bakes: &'a [String]) -> Self {
        self.bakes = bakes;
        self
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FabricatorKey {
    executable: PathBuf,
    active_bakes: Vec<String>,
}

impl FabricatorKey {
    fn from_request(request: FabricateRequest<'_>) -> Self {
        let executable = request
            .executable
            .map_or_else(|| PathBuf::from("node"), Path::to_path_buf);
        Self {
            executable,
            active_bakes: active_bakes(request.bakes),
        }
    }

    fn command_label(&self) -> String {
        self.executable.display().to_string()
    }
}

#[derive(Debug)]
struct FabricatorChild {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl FabricatorChild {
    fn spawn(key: &FabricatorKey) -> Result<Self, PkgError> {
        let mut child = Command::new(&key.executable)
            .args(&key.active_bakes)
            .arg("-e")
            .arg(BYTECODE_FABRICATOR_SCRIPT)
            .env("PKG_EXECPATH", "PKG_INVOKE_NODEJS")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|source| PkgError::Io {
                path: key.command_label(),
                source,
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| PkgError::Pack("node bytecode stdin was not available".to_owned()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| PkgError::Pack("node bytecode stdout was not available".to_owned()))?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    fn fabricate(&mut self, request: FabricateRequest<'_>) -> Result<Vec<u8>, PkgError> {
        write_request(&mut self.stdin, request.snap, request.source)?;
        let bytes = read_response(&mut self.stdout, request.snap)?;
        if bytes.is_empty() {
            return Err(PkgError::Pack(format!(
                "failed to make bytecode for {}: empty cached data",
                request.snap
            )));
        }
        Ok(bytes)
    }
}

impl Drop for FabricatorChild {
    fn drop(&mut self) {
        let _ignored = self.child.kill();
        let _ignored = self.child.wait();
    }
}

/// Generate V8 cached bytecode for a JavaScript source blob.
pub fn fabricate(
    pool: &mut FabricatorPool,
    request: FabricateRequest<'_>,
) -> Result<Vec<u8>, PkgError> {
    pool.fabricate(request)
}

/// Generate bytecode, retrying once if the first fabrication fails.
///
/// This preserves the JavaScript fabricator behavior: return the first
/// successful buffer, but retry once for runtimes that fail the first compile.
pub fn fabricate_twice(
    pool: &mut FabricatorPool,
    request: FabricateRequest<'_>,
) -> Result<Vec<u8>, PkgError> {
    match fabricate(pool, request) {
        Ok(bytes) => Ok(bytes),
        Err(_error) => fabricate(pool, request),
    }
}

/// Shut down retained fabricator processes.
pub fn shutdown_fabricators(pool: &mut FabricatorPool) {
    pool.clear();
}

fn active_bakes(bakes: &[String]) -> Vec<String> {
    bakes
        .iter()
        .filter(|bake| {
            let normalized = bake.replace('_', "-");
            !INERT_BAKES.contains(&normalized.as_str())
        })
        .cloned()
        .collect()
}

fn write_request(stdin: &mut ChildStdin, snap: &str, source: &[u8]) -> Result<(), PkgError> {
    write_len(stdin, snap.len(), "snapshot")?;
    stdin
        .write_all(snap.as_bytes())
        .map_err(|source| PkgError::Io {
            path: "node stdin".to_owned(),
            source,
        })?;
    write_len(stdin, source.len(), "source")?;
    stdin.write_all(source).map_err(|source| PkgError::Io {
        path: "node stdin".to_owned(),
        source,
    })?;
    stdin.flush().map_err(|source| PkgError::Io {
        path: "node stdin".to_owned(),
        source,
    })
}

fn write_len(stdin: &mut ChildStdin, len: usize, label: &str) -> Result<(), PkgError> {
    let len = i32::try_from(len)
        .map_err(|_error| PkgError::Pack(format!("{label} is too large to fabricate")))?;
    stdin
        .write_all(&len.to_le_bytes())
        .map_err(|source| PkgError::Io {
            path: "node stdin".to_owned(),
            source,
        })
}

fn read_response(stdout: &mut BufReader<ChildStdout>, snap: &str) -> Result<Vec<u8>, PkgError> {
    let mut header = [0_u8; 4];
    stdout.read_exact(&mut header).map_err(|source| {
        PkgError::Pack(format!(
            "failed to make bytecode for {snap}: fabricator closed before response ({source})"
        ))
    })?;
    let len = i32::from_le_bytes(header);
    if len < 0 {
        return Err(PkgError::Pack(format!(
            "failed to make bytecode for {snap}: negative cached data length"
        )));
    }

    let mut bytes = vec![0_u8; len as usize];
    stdout.read_exact(&mut bytes).map_err(|source| {
        PkgError::Pack(format!(
            "failed to make bytecode for {snap}: fabricator closed during response ({source})"
        ))
    })?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    fn test_temp_dir(name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "pkg-rust-fabricate-{name}-{}-{now}",
            std::process::id()
        ));
        let _ignored = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir)?;
        Ok(temp_dir)
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let mut permissions = std::fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions)?;
        Ok(())
    }

    #[cfg(unix)]
    fn write_framed_fabricator(
        temp_dir: &Path,
    ) -> Result<(PathBuf, PathBuf, PathBuf), Box<dyn std::error::Error>> {
        let spawn_log = temp_dir.join("spawns.log");
        let args_log = temp_dir.join("args.log");
        let request_log = temp_dir.join("requests.log");
        let handler = temp_dir.join("handler.js");
        std::fs::write(
            &handler,
            format!(
                r#"
const fs = require('fs');
fs.appendFileSync({spawn_log:?}, 'spawn\n');
fs.writeFileSync({args_log:?}, process.argv.slice(2).join('\n'));
let stdin = Buffer.alloc(0);
process.stdin.on('data', (chunk) => {{
  stdin = Buffer.concat([stdin, chunk]);
  while (stdin.length >= 4) {{
    const snapLength = stdin.readInt32LE(0);
    if (stdin.length < 4 + snapLength + 4) return;
    const bodyLength = stdin.readInt32LE(4 + snapLength);
    if (stdin.length < 4 + snapLength + 4 + bodyLength) return;
    const snap = stdin.toString('utf8', 4, 4 + snapLength);
    const bodyStart = 4 + snapLength + 4;
    const body = stdin.subarray(bodyStart, bodyStart + bodyLength).toString('utf8');
    stdin = stdin.subarray(bodyStart + bodyLength);
    fs.appendFileSync({request_log:?}, `${{snap}}:${{body}}\n`);
    const payload = Buffer.from(`BYTECODE:${{snap}}:${{body}}`);
    const header = Buffer.alloc(4);
    header.writeInt32LE(payload.length, 0);
    process.stdout.write(header);
    process.stdout.write(payload);
  }}
}});
process.stdin.resume();
"#,
                spawn_log = spawn_log.display().to_string(),
                args_log = args_log.display().to_string(),
                request_log = request_log.display().to_string()
            ),
        )?;
        let executable = temp_dir.join("fake-node");
        std::fs::write(
            &executable,
            format!("#!/bin/sh\nexec node '{}' \"$@\"\n", handler.display()),
        )?;
        make_executable(&executable)?;
        Ok((executable, args_log, request_log))
    }

    #[cfg(unix)]
    #[test]
    fn reuses_process_and_filters_inert_bakes() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = test_temp_dir("pool")?;
        let (executable, args_log, _request_log) = write_framed_fabricator(&temp_dir)?;
        let bakes = vec![
            "--prof".to_owned(),
            "--trace_opt".to_owned(),
            "--max-old-space-size=64".to_owned(),
        ];

        let mut pool = FabricatorPool::new();
        let first = fabricate(
            &mut pool,
            FabricateRequest::new("/snapshot/one.js", b"one")
                .with_executable(&executable)
                .with_bakes(&bakes),
        )?;
        let second = fabricate(
            &mut pool,
            FabricateRequest::new("/snapshot/two.js", b"two")
                .with_executable(&executable)
                .with_bakes(&bakes),
        )?;
        shutdown_fabricators(&mut pool);

        assert_eq!(first, b"BYTECODE:/snapshot/one.js:one");
        assert_eq!(second, b"BYTECODE:/snapshot/two.js:two");

        let spawns = std::fs::read_to_string(temp_dir.join("spawns.log"))?;
        assert_eq!(spawns.lines().count(), 1);
        let args = std::fs::read_to_string(args_log)?;
        assert!(args.contains("--max-old-space-size=64"));
        assert!(!args.contains("--prof"));
        assert!(!args.contains("--trace_opt"));
        assert!(!args.contains("--trace-opt"));

        std::fs::remove_dir_all(temp_dir)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn fabricate_twice_returns_first_success() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = test_temp_dir("twice-success")?;
        let (executable, _args_log, request_log) = write_framed_fabricator(&temp_dir)?;

        let mut pool = FabricatorPool::new();
        let bytes = fabricate_twice(
            &mut pool,
            FabricateRequest::new("/snapshot/app.js", b"module.exports = 42;")
                .with_executable(&executable),
        )?;
        shutdown_fabricators(&mut pool);

        assert_eq!(bytes, b"BYTECODE:/snapshot/app.js:module.exports = 42;");
        let requests = std::fs::read_to_string(request_log)?;
        assert_eq!(requests.lines().count(), 1);

        std::fs::remove_dir_all(temp_dir)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn fabricate_twice_retries_after_failure() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = test_temp_dir("twice-retry")?;
        let (delegate, _args_log, request_log) = write_framed_fabricator(&temp_dir)?;
        let state = temp_dir.join("first-failure");
        let executable = temp_dir.join("flaky-node");
        std::fs::write(
            &executable,
            format!(
                "#!/bin/sh\nif [ ! -f '{state}' ]; then touch '{state}'; exit 7; fi\nexec '{}' \"$@\"\n",
                delegate.display(),
                state = state.display()
            ),
        )?;
        make_executable(&executable)?;

        let mut pool = FabricatorPool::new();
        let bytes = fabricate_twice(
            &mut pool,
            FabricateRequest::new("/snapshot/retry.js", b"retry").with_executable(&executable),
        )?;
        shutdown_fabricators(&mut pool);

        assert_eq!(bytes, b"BYTECODE:/snapshot/retry.js:retry");
        let requests = std::fs::read_to_string(request_log)?;
        assert_eq!(requests.lines().count(), 1);

        std::fs::remove_dir_all(temp_dir)?;
        Ok(())
    }
}
