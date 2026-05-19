const ORIGINAL_PACKAGE_VERSION: &str = "5.8.1";
const BOOTSTRAP_SOURCE: &str = include_str!("../../prelude/bootstrap.js");
const DIAGNOSTIC_SOURCE: &str = include_str!("../../prelude/diagnostic.js");

/// Build the JavaScript producer prelude template.
///
/// The returned template still contains producer placeholders such as
/// `%VIRTUAL_FILESYSTEM%`; pass it to [`crate::render_prelude`] with a producer
/// manifest before writing an executable image.
///
/// # Example
///
/// ```
/// let template = pkg_rust::prelude_template(false);
/// assert!(template.contains("%VIRTUAL_FILESYSTEM%"));
/// assert!(!template.contains("%VERSION%"));
/// ```
#[must_use]
pub fn prelude_template(debug: bool) -> String {
    // DECISION: during the migration, read the original runtime bootstrap from
    // the parent JS repo instead of copying it into rust-port; this preserves
    // runtime parity without vendoring the JS source inside the Rust crate.
    let bootstrap = BOOTSTRAP_SOURCE.replace("%VERSION%", ORIGINAL_PACKAGE_VERSION);
    let diagnostic = if debug { DIAGNOSTIC_SOURCE } else { "" };
    format!(
        "return (function (REQUIRE_COMMON, VIRTUAL_FILESYSTEM, DEFAULT_ENTRYPOINT, SYMLINKS, DICT, DOCOMPRESS) {{\n        {bootstrap}{diagnostic}\n}})(function (exports) {{\n{}\n}},\n%VIRTUAL_FILESYSTEM%\n,\n%DEFAULT_ENTRYPOINT%\n,\n%SYMLINKS%\n,\n%DICT%\n,\n%DOCOMPRESS%\n);",
        common_runtime_source()
    )
}

fn common_runtime_source() -> &'static str {
    r#"
const path = require('path');
const win32 = process.platform === 'win32';
const hasURL = typeof URL !== 'undefined';

exports.STORE_BLOB = 0;
exports.STORE_CONTENT = 1;
exports.STORE_LINKS = 2;
exports.STORE_STAT = 3;

function uppercaseDriveLetter(f) {
  if (f.slice(1, 3) !== ':\\') return f;
  return f[0].toUpperCase() + f.slice(1);
}

function removeTrailingSlashes(f) {
  if (f === '/') return f;
  if (f.slice(1) === ':\\') return f;

  let last = f.length - 1;
  while (true) {
    const char = f.charAt(last);
    if (char === '\\' || char === '/') {
      f = f.slice(0, -1);
      last -= 1;
    } else {
      break;
    }
  }
  return f;
}

const isUrl = (p) => hasURL && p instanceof URL;

function pathToString(p, win) {
  if (Buffer.isBuffer(p)) return p.toString();
  if (isUrl(p)) return win ? p.pathname.replace(/^\//, '') : p.pathname;
  return p;
}

exports.isRootPath = function isRootPath(p) {
  let file = pathToString(p, false);
  if (file === '.') file = path.resolve(file);
  return path.dirname(file) === p;
};

exports.normalizePath = function normalizePath(f) {
  let file = pathToString(f, win32);
  if (!/^.:$/.test(file)) file = path.normalize(file);
  if (win32) file = uppercaseDriveLetter(file);
  return removeTrailingSlashes(file);
};

exports.insideSnapshot = function insideSnapshot(f) {
  f = pathToString(f, win32);

  if (typeof f !== 'string') {
    return false;
  }

  if (win32) {
    const slice112 = f.slice(1, 12);

    return (
      slice112 === ':\\snapshot\\' ||
      slice112 === ':/snapshot\\' ||
      slice112 === ':\\snapshot/' ||
      slice112 === ':/snapshot/' ||
      slice112 === ':\\snapshot' ||
      slice112 === ':/snapshot'
    );
  }

  const slice010 = f.slice(0, 10);

  return slice010 === '/snapshot/' || slice010 === '/snapshot';
};

exports.stripSnapshot = function stripSnapshot(f) {
  const file = exports.normalizePath(f);

  if (/^.:\\snapshot$/.test(file)) {
    return `${file[0]}:\\**\\`;
  }

  if (/^.:\\snapshot\\/.test(file)) {
    return `${file[0]}:\\**${file.slice(11)}`;
  }

  if (/^\/snapshot$/.test(file)) {
    return '/**/';
  }

  if (/^\/snapshot\//.test(file)) {
    return `/**${file.slice(9)}`;
  }

  return f;
};

exports.removeUplevels = function removeUplevels(f) {
  if (win32) {
    while (true) {
      if (f.slice(0, 3) === '..\\') {
        f = f.slice(3);
      } else if (f === '..') {
        f = '.';
      } else {
        break;
      }
    }

    return f;
  }

  while (true) {
    if (f.slice(0, 3) === '../') {
      f = f.slice(3);
    } else if (f === '..') {
      f = '.';
    } else {
      break;
    }
  }

  return f;
};
"#
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::common_runtime_source;

    #[test]
    fn generated_common_helper_matches_posix_snapshot_behavior()
    -> Result<(), Box<dyn std::error::Error>> {
        let script = format!(
            r#"
const common = {{}};
(function (exports) {{
{}
}})(common);

const checks = [
  ['inside snapshot root', common.insideSnapshot('/snapshot'), true],
  ['inside snapshot child', common.insideSnapshot('/snapshot/app.js'), true],
  ['outside snapshot prefix', common.insideSnapshot('/snapshoter/app.js'), false],
  ['strip snapshot root', common.stripSnapshot('/snapshot'), '/**/'],
  ['strip snapshot child', common.stripSnapshot('/snapshot/app.js'), '/**/app.js'],
  ['strip snapshot normalized child', common.stripSnapshot('/snapshot//foo//bar/\\//'), '/**/foo/bar'],
  ['remove leading uplevel', common.removeUplevels('../../foo'), 'foo'],
  ['preserve internal uplevel', common.removeUplevels('../foo/../bar'), 'foo/../bar'],
  ['dot slash is unchanged', common.removeUplevels('./foo'), './foo'],
  ['only uplevels collapse to dot', common.removeUplevels('../..'), '.'],
];

for (const [name, actual, expected] of checks) {{
  if (actual !== expected) {{
    console.error(`${{name}}: expected ${{JSON.stringify(expected)}}, got ${{JSON.stringify(actual)}}`);
    process.exit(1);
  }}
}}
"#,
            common_runtime_source()
        );

        let output = Command::new("node").arg("-e").arg(script).output()?;
        assert!(
            output.status.success(),
            "generated common helper failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }
}
