use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::PkgError;

const BYTECODE_FABRICATOR_SCRIPT: &str = r#"
const vm = require('vm');
const module = require('module');
const snap = process.argv[1];
const chunks = [];

process.stdin.on('data', (chunk) => chunks.push(chunk));
process.stdin.on('end', () => {
  const body = Buffer.concat(chunks);
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

  process.stdout.write(script.cachedData);
});
"#;

/// Process-backed bytecode fabricator state.
///
/// The current port starts one Node process per request. The explicit pool type
/// keeps the public seam aligned with the JavaScript fabricator and leaves room
/// for a long-lived target-binary pool without changing callers.
#[derive(Clone, Debug, Default)]
pub struct FabricatorPool {
    _private: (),
}

impl FabricatorPool {
    /// Build an empty fabricator pool.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Clear any retained fabricator state.
    pub fn clear(&mut self) {}
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
}

impl<'a> FabricateRequest<'a> {
    /// Build a request using host `node` as the fabricator executable.
    #[must_use]
    pub fn new(snap: &'a str, source: &'a [u8]) -> Self {
        Self {
            snap,
            source,
            executable: None,
        }
    }

    /// Use an explicit target Node executable for this request.
    #[must_use]
    pub fn with_executable(mut self, executable: &'a Path) -> Self {
        self.executable = Some(executable);
        self
    }
}

/// Generate V8 cached bytecode for a JavaScript source blob.
pub fn fabricate(
    pool: &mut FabricatorPool,
    request: FabricateRequest<'_>,
) -> Result<Vec<u8>, PkgError> {
    let _pool = pool;
    let mut command = match request.executable {
        Some(path) => Command::new(path),
        None => Command::new("node"),
    };
    let command_label = request
        .executable
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "node".to_owned());
    let mut child = command
        .arg("-e")
        .arg(BYTECODE_FABRICATOR_SCRIPT)
        .arg(request.snap)
        .env("PKG_EXECPATH", "PKG_INVOKE_NODEJS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| PkgError::Io {
            path: command_label.clone(),
            source,
        })?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| PkgError::Pack("node bytecode stdin was not available".to_owned()))?;
    stdin
        .write_all(request.source)
        .map_err(|source| PkgError::Io {
            path: "node stdin".to_owned(),
            source,
        })?;
    drop(stdin);

    let output = child.wait_with_output().map_err(|source| PkgError::Io {
        path: command_label,
        source,
    })?;
    if !output.status.success() {
        return Err(PkgError::Pack(format!(
            "failed to make bytecode for {}: {}",
            request.snap,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    if output.stdout.is_empty() {
        return Err(PkgError::Pack(format!(
            "failed to make bytecode for {}: empty cached data",
            request.snap
        )));
    }

    Ok(output.stdout)
}

/// Generate bytecode twice and return the second result.
///
/// This preserves the JavaScript fabricator seam used by callers that need a
/// warm-up compile before taking the final cached-data payload.
pub fn fabricate_twice(
    pool: &mut FabricatorPool,
    request: FabricateRequest<'_>,
) -> Result<Vec<u8>, PkgError> {
    let _first = fabricate(pool, request)?;
    fabricate(pool, request)
}

/// Shut down retained fabricator processes.
pub fn shutdown_fabricators(pool: &mut FabricatorPool) {
    pool.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn explicit_executable_is_used_for_fabrication() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir =
            std::env::temp_dir().join(format!("pkg-rust-fabricate-path-{}", std::process::id()));
        let _ignored = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir)?;
        let executable = temp_dir.join("fake-node");
        std::fs::write(
            &executable,
            "#!/bin/sh\ncat >/dev/null\nprintf TARGET_BYTECODE\n",
        )?;
        let mut permissions = std::fs::metadata(&executable)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions)?;

        let mut pool = FabricatorPool::new();
        let bytes = fabricate(
            &mut pool,
            FabricateRequest::new("/snapshot/app.js", b"module.exports = 42;")
                .with_executable(&executable),
        )?;

        assert_eq!(bytes, b"TARGET_BYTECODE");

        std::fs::remove_dir_all(temp_dir)?;
        Ok(())
    }
}
