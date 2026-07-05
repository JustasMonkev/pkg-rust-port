//! ESM-to-CommonJS transformation, ported from yao-pkg `lib/esm-transformer.ts`.
//!
//! The JS implementation analyzes modules with Babel and transforms them with
//! esbuild. The Rust port performs both steps with SWC: analysis through the
//! same parser used by source detection, and transformation through SWC's
//! `common_js` pass, which also rewrites `import.meta.url` / `.filename` /
//! `.dirname` to their CommonJS equivalents natively (the JS implementation
//! patches esbuild output with a shim for the same effect).

use std::path::Path;
use std::sync::OnceLock;

use swc_common::{FileName, GLOBALS, Globals, Mark, SourceMap, sync::Lrc};
use swc_ecma_ast::{Callee, EsVersion, Expr, Lit, Module, ModuleDecl, ModuleItem, Program, Str};
use swc_ecma_parser::{EsSyntax, Parser, StringInput, Syntax, lexer::Lexer};
use swc_ecma_transforms_base::fixer::fixer;
use swc_ecma_transforms_base::helpers::{HELPERS, Helpers, inject_helpers};
use swc_ecma_transforms_base::hygiene::hygiene;
use swc_ecma_transforms_base::resolver;
use swc_ecma_transforms_module::common_js::{FeatureFlag, common_js};
use swc_ecma_transforms_module::path::Resolver as ImportPathResolver;
use swc_ecma_visit::{Visit, VisitMut, VisitMutWith, VisitWith};

/// Result of attempting an ESM-to-CJS transformation.
pub(crate) struct EsmTransform {
    /// Output source: transformed CJS, or the original input when
    /// `is_transformed` is false.
    pub(crate) code: String,
    /// Whether the code was actually transformed.
    pub(crate) is_transformed: bool,
    /// Warning to surface through the package warning channel, if any.
    pub(crate) warning: Option<String>,
}

impl EsmTransform {
    fn untransformed(code: &str, warning: Option<String>) -> Self {
        Self {
            code: code.to_owned(),
            is_transformed: false,
            warning,
        }
    }
}

/// JS `common.isESMFile`: `.mjs` is ESM, `.cjs` is CJS, `.js` follows the
/// nearest `package.json` `"type": "module"` marker.
pub(crate) fn is_esm_file(path: &Path) -> bool {
    let extension = path.extension().and_then(|extension| extension.to_str());
    match extension {
        Some("mjs") => return true,
        Some("cjs") => return false,
        Some("js") => {}
        _ => return false,
    }
    let mut current = path.parent();
    while let Some(directory) = current {
        let package_json = directory.join("package.json");
        if package_json.is_file() {
            return std::fs::read_to_string(&package_json)
                .ok()
                .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
                .and_then(|json| {
                    json.get("type")
                        .and_then(|value| value.as_str())
                        .map(|value| value == "module")
                })
                .unwrap_or(false);
        }
        current = directory.parent();
    }
    false
}

/// JS `common.unlikelyJavascript` plus the `.d.ts` compound-extension check.
fn unlikely_javascript(path: &Path) -> bool {
    if path
        .to_string_lossy()
        .to_ascii_lowercase()
        .ends_with(".d.ts")
    {
        return true;
    }
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("css" | "html" | "json" | "vue")
    )
}

/// JS `rewriteMjsRequirePaths`: relative `require("./x.mjs")` calls in
/// transformed output are rewritten to `.js` because the packer renames
/// transformed `.mjs` snapshots to `.js`.
pub(crate) fn rewrite_mjs_require_paths(code: &str) -> String {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        regex::Regex::new(r#"require\((["'])(\.\.?/[^"']*?)\.mjs(["'])\)"#)
            .unwrap_or_else(|_| regex::Regex::new("$^").unwrap_or_else(|_| unreachable!()))
    });
    pattern
        .replace_all(code, |captures: &regex::Captures<'_>| {
            let open = &captures[1];
            let path = &captures[2];
            let close = &captures[3];
            if open == close {
                format!("require({open}{path}.js{close})")
            } else {
                captures[0].to_owned()
            }
        })
        .into_owned()
}

/// JS `transformESMtoCJS`: convert an ESM module to CommonJS so it can be
/// compiled to bytecode. Top-level await without exports is wrapped in an
/// async IIFE (imports hoisted above the wrapper); top-level await combined
/// with exports cannot be transformed and ships untransformed with a warning.
pub(crate) fn transform_esm_to_cjs(code: &str, filename: &Path) -> EsmTransform {
    if unlikely_javascript(filename) {
        return EsmTransform::untransformed(code, None);
    }

    let analysis = analyze_module(code);
    let mut code_to_transform = code.to_owned();
    if let Some(analysis) = &analysis
        && analysis.has_top_level_await
    {
        if analysis.has_exports {
            return EsmTransform::untransformed(
                code,
                Some(format!(
                    "Module {} has both top-level await and export statements. This combination cannot be safely transformed to CommonJS in pkg's ESM transformer. The original source code will be used as-is; depending on the package visibility and build configuration, bytecode compilation may fail and the module may need to be loaded from source or be skipped.",
                    filename.display()
                )),
            );
        }
        code_to_transform = wrap_in_async_iife(code, &analysis.import_spans);
    }

    match transform_module_source(&code_to_transform, filename.parent()) {
        Ok(output) => EsmTransform {
            code: output,
            is_transformed: true,
            warning: None,
        },
        Err(message) => EsmTransform::untransformed(
            code,
            Some(format!(
                "Failed to transform ESM to CJS for {}: {message}",
                filename.display()
            )),
        ),
    }
}

struct ModuleAnalysis {
    has_top_level_await: bool,
    has_exports: bool,
    /// Byte ranges of import declarations within the source.
    import_spans: Vec<(usize, usize)>,
}

fn analyze_module(code: &str) -> Option<ModuleAnalysis> {
    let (module, base_offset) = parse_esm_module(code).ok()?;

    let mut has_exports = false;
    let mut import_spans = Vec::new();
    for item in &module.body {
        match item {
            ModuleItem::ModuleDecl(ModuleDecl::Import(import)) => {
                let lo = usize::try_from(import.span.lo.0).ok()?;
                let hi = usize::try_from(import.span.hi.0).ok()?;
                import_spans.push((
                    lo.saturating_sub(base_offset),
                    hi.saturating_sub(base_offset),
                ));
            }
            ModuleItem::ModuleDecl(
                ModuleDecl::ExportDecl(_)
                | ModuleDecl::ExportNamed(_)
                | ModuleDecl::ExportDefaultDecl(_)
                | ModuleDecl::ExportDefaultExpr(_)
                | ModuleDecl::ExportAll(_),
            ) => {
                has_exports = true;
            }
            _ => {}
        }
    }

    let mut await_finder = TopLevelAwaitFinder::default();
    module.visit_with(&mut await_finder);

    Some(ModuleAnalysis {
        has_top_level_await: await_finder.found,
        has_exports,
        import_spans,
    })
}

/// Visitor that finds `await` (or `for await ... of`) outside any function
/// body, mirroring the parent-walk in the JS `detectESMFeatures`.
#[derive(Default)]
struct TopLevelAwaitFinder {
    found: bool,
}

impl Visit for TopLevelAwaitFinder {
    fn visit_await_expr(&mut self, node: &swc_ecma_ast::AwaitExpr) {
        self.found = true;
        node.visit_children_with(self);
    }

    fn visit_for_of_stmt(&mut self, node: &swc_ecma_ast::ForOfStmt) {
        if node.is_await {
            self.found = true;
        }
        node.visit_children_with(self);
    }

    fn visit_function(&mut self, _node: &swc_ecma_ast::Function) {}

    fn visit_arrow_expr(&mut self, _node: &swc_ecma_ast::ArrowExpr) {}
}

/// JS `ASYNC_IIFE_WRAPPER` handling: hoist import declarations above the
/// wrapper and wrap the remaining source in `(async () => { ... })()`.
///
/// The JS implementation hoists whole physical lines; this port splices the
/// exact import-declaration byte ranges instead, so statements sharing a line
/// with an import (e.g. minified `import x from 'x'; await start()`) stay
/// inside the async wrapper.
fn wrap_in_async_iife(code: &str, import_spans: &[(usize, usize)]) -> String {
    let mut spans: Vec<(usize, usize)> = import_spans
        .iter()
        .copied()
        .filter(|(lo, hi)| lo < hi && *hi <= code.len())
        .collect();
    spans.sort_unstable();
    if spans.is_empty() {
        return format!("(async () => {{\n{code}\n}})()");
    }
    let mut imports = String::new();
    let mut rest = String::new();
    let mut cursor = 0;
    for (lo, hi) in spans {
        if lo < cursor {
            continue;
        }
        rest.push_str(&code[cursor..lo]);
        imports.push_str(&code[lo..hi]);
        // Each hoisted declaration ends its own line; ASI keeps declarations
        // without a trailing semicolon well-formed.
        imports.push('\n');
        cursor = hi;
    }
    rest.push_str(&code[cursor..]);
    format!("{imports}\n(async () => {{\n{rest}\n}})()")
}

fn parse_esm_module(code: &str) -> Result<(Module, usize), String> {
    let cm: Lrc<SourceMap> = Default::default();
    let file = cm.new_source_file(FileName::Custom("module.js".into()).into(), code.to_owned());
    let base_offset = usize::try_from(file.start_pos.0).map_err(|error| error.to_string())?;
    let lexer = Lexer::new(
        Syntax::Es(EsSyntax {
            decorators: true,
            ..Default::default()
        }),
        EsVersion::latest(),
        StringInput::from(&*file),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let module = parser
        .parse_module()
        .map_err(|error| format!("{error:?}"))?;
    if let Some(error) = parser.take_errors().into_iter().next() {
        return Err(format!("{error:?}"));
    }
    Ok((module, base_offset))
}

/// Rewrites bare specifiers that only resolve through the `import` exports
/// condition (ESM-only packages) into relative paths before the CommonJS
/// emit. The emit turns every import into `require()`, and Node's runtime
/// resolver rejects such bare specifiers with `ERR_PACKAGE_PATH_NOT_EXPORTED`
/// because the package exposes no `require` condition; the relative path
/// loads the exact file the walker packages instead. Specifiers `require()`
/// can already resolve are left untouched, so ordinary packages keep their
/// bare form. Relative `.mjs` targets are later renamed to `.js` by the
/// shared `rewrite_mjs_require_paths` pass, matching the packer's snapshot
/// renames.
struct EsmOnlySpecifierRewriter<'a> {
    base_dir: &'a Path,
}

impl EsmOnlySpecifierRewriter<'_> {
    fn rewrite(&self, source: &mut Str) {
        let Some(specifier) = source.value.as_str() else {
            return;
        };
        // Relative/absolute paths and scheme-prefixed specifiers (node:,
        // file:, data:) are never exports-mapped package names.
        if specifier.starts_with('.') || specifier.starts_with('/') || specifier.contains(':') {
            return;
        }
        let Some(resolved) = crate::resolve::esm_only_import_resolution(specifier, self.base_dir)
        else {
            return;
        };
        // The resolution is canonicalized; canonicalize the base too so
        // symlinked segments (macOS `/tmp`) don't poison the relative path.
        let base_dir = self
            .base_dir
            .canonicalize()
            .unwrap_or_else(|_| self.base_dir.to_path_buf());
        let Some(relative) = relative_specifier(&base_dir, &resolved) else {
            return;
        };
        source.value = relative.into();
        source.raw = None;
    }
}

impl VisitMut for EsmOnlySpecifierRewriter<'_> {
    fn visit_mut_import_decl(&mut self, node: &mut swc_ecma_ast::ImportDecl) {
        self.rewrite(&mut node.src);
    }

    fn visit_mut_named_export(&mut self, node: &mut swc_ecma_ast::NamedExport) {
        if let Some(src) = &mut node.src {
            self.rewrite(src);
        }
    }

    fn visit_mut_export_all(&mut self, node: &mut swc_ecma_ast::ExportAll) {
        self.rewrite(&mut node.src);
    }

    fn visit_mut_call_expr(&mut self, node: &mut swc_ecma_ast::CallExpr) {
        node.visit_mut_children_with(self);
        if matches!(node.callee, Callee::Import(_))
            && let Some(argument) = node.args.first_mut()
            && argument.spread.is_none()
            && let Expr::Lit(Lit::Str(source)) = &mut *argument.expr
        {
            self.rewrite(source);
        }
    }
}

/// Render `to` as a `require()`-style specifier relative to `from_dir`,
/// using forward slashes (Node accepts them on every platform).
fn relative_specifier(from_dir: &Path, to: &Path) -> Option<String> {
    let from: Vec<_> = from_dir.components().collect();
    let to_components: Vec<_> = to.components().collect();
    let common = from
        .iter()
        .zip(&to_components)
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<&str> = vec![".."; from.len() - common];
    for component in &to_components[common..] {
        parts.push(component.as_os_str().to_str()?);
    }
    let joined = parts.join("/");
    Some(if joined.starts_with("..") {
        joined
    } else {
        format!("./{joined}")
    })
}

fn transform_module_source(code: &str, base_dir: Option<&Path>) -> Result<String, String> {
    let cm: Lrc<SourceMap> = Default::default();
    let file = cm.new_source_file(FileName::Custom("module.js".into()).into(), code.to_owned());
    let lexer = Lexer::new(
        Syntax::Es(EsSyntax {
            decorators: true,
            ..Default::default()
        }),
        EsVersion::latest(),
        StringInput::from(&*file),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let module = parser
        .parse_module()
        .map_err(|error| format!("{error:?}"))?;
    if let Some(error) = parser.take_errors().into_iter().next() {
        return Err(format!("{error:?}"));
    }

    let globals = Globals::new();
    let mut program = Program::Module(module);
    if let Some(base_dir) = base_dir {
        program.visit_mut_with(&mut EsmOnlySpecifierRewriter { base_dir });
    }
    GLOBALS.set(&globals, || {
        // `false` inlines the interop helpers into the output, so the
        // transformed module stays self-contained like esbuild's CJS output.
        HELPERS.set(&Helpers::new(false), || {
            let unresolved_mark = Mark::new();
            let top_level_mark = Mark::new();
            program.mutate(resolver(unresolved_mark, top_level_mark, false));
            program.mutate(common_js(
                ImportPathResolver::Default,
                unresolved_mark,
                Default::default(),
                // The produced executables target Node 14+; arrows and block
                // scoping are always available, matching esbuild target node20.
                FeatureFlag {
                    support_block_scoping: true,
                    support_arrow: true,
                },
            ));
            // The interop helper calls emitted by `common_js` (for example
            // `_interop_require_default`) are only references; this pass
            // prepends their function definitions to keep the output
            // self-contained.
            program.mutate(inject_helpers(unresolved_mark));
            // hygiene + fixer finish the standard SWC pipeline: hygiene
            // resolves mark-based renames and fixer restores required
            // parentheses (e.g. `(0, _mod.fn)()`) before code generation.
            program.mutate(hygiene());
            program.mutate(fixer(None));
        });
    });

    let mut buffer = Vec::new();
    {
        let writer =
            swc_ecma_codegen::text_writer::JsWriter::new(cm.clone(), "\n", &mut buffer, None);
        let mut emitter = swc_ecma_codegen::Emitter {
            cfg: Default::default(),
            cm,
            comments: None,
            wr: writer,
        };
        emitter
            .emit_program(&program)
            .map_err(|error| error.to_string())?;
    }
    String::from_utf8(buffer).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{rewrite_mjs_require_paths, transform_esm_to_cjs};

    #[test]
    fn transforms_esm_imports_and_exports_to_cjs() {
        let result = transform_esm_to_cjs(
            "import { readFile } from 'fs';\nexport const answer = 42;\n",
            Path::new("/app/module.mjs"),
        );
        assert!(result.is_transformed);
        assert!(result.code.contains("require(\"fs\")"));
        assert!(result.code.contains("exports"));
        assert!(!result.code.contains("import {"));
    }

    #[test]
    fn rewrites_import_meta_members_to_cjs_equivalents() {
        let result = transform_esm_to_cjs(
            "export const here = import.meta.dirname;\nexport const me = import.meta.filename;\nconsole.log(import.meta.url);\n",
            Path::new("/app/module.mjs"),
        );
        assert!(result.is_transformed);
        assert!(result.code.contains("__dirname"));
        assert!(result.code.contains("__filename"));
        assert!(!result.code.contains("import.meta"));
    }

    #[test]
    fn wraps_top_level_await_without_exports_in_async_iife() {
        let result = transform_esm_to_cjs(
            "import fs from 'fs';\nconst data = await Promise.resolve(1);\nconsole.log(data, fs ? 1 : 0);\n",
            Path::new("/app/module.mjs"),
        );
        assert!(result.is_transformed);
        assert!(result.code.contains("async ()"));
        assert!(result.code.contains("require(\"fs\")"));
    }

    #[test]
    fn statements_sharing_an_import_line_stay_inside_the_async_wrapper() {
        // Minified-style ESM: the import shares a physical line with the
        // top-level await. The await must end up inside the async IIFE.
        let result = transform_esm_to_cjs(
            "import { start } from 'fs'; const out = await Promise.resolve(start ? 1 : 2); console.log(out);",
            Path::new("/app/module.mjs"),
        );
        assert!(result.is_transformed);
        let async_at = result.code.find("async").unwrap_or(usize::MAX);
        let await_at = result.code.find("await").unwrap_or(usize::MAX);
        assert!(
            async_at < await_at && await_at != usize::MAX,
            "await must be wrapped by the async IIFE: {}",
            result.code
        );
        assert!(result.code.contains("require(\"fs\")"));
    }

    #[test]
    fn top_level_await_with_exports_ships_untransformed_with_warning() {
        let result = transform_esm_to_cjs(
            "export const data = 1;\nawait Promise.resolve(1);\n",
            Path::new("/app/module.mjs"),
        );
        assert!(!result.is_transformed);
        assert!(
            result
                .warning
                .is_some_and(|warning| warning.contains("top-level await and export statements"))
        );
    }

    #[test]
    fn default_import_inlines_interop_helper_definition() {
        let result = transform_esm_to_cjs(
            "import dep from 'fs';\nconsole.log(dep);\n",
            Path::new("/app/module.mjs"),
        );
        assert!(result.is_transformed);
        assert!(result.code.contains("_interop_require_default"));
        assert!(
            result.code.contains("function _interop_require_default"),
            "interop helper definition must be inlined, not just referenced: {}",
            result.code
        );
    }

    #[test]
    fn namespace_import_inlines_wildcard_helper_definition() {
        let result = transform_esm_to_cjs(
            "import * as ns from 'fs';\nconsole.log(ns);\n",
            Path::new("/app/module.mjs"),
        );
        assert!(result.is_transformed);
        assert!(
            result.code.contains("function _interop_require_wildcard"),
            "wildcard helper definition must be inlined, not just referenced: {}",
            result.code
        );
    }

    #[test]
    fn rewrites_bare_esm_only_imports_to_relative_paths() -> Result<(), std::io::Error> {
        let root = std::env::temp_dir().join(format!("pkg-rust-esm-only-{}", std::process::id()));
        let _ignored = std::fs::remove_dir_all(&root);
        let package_dir = root.join("node_modules/esmpkg");
        std::fs::create_dir_all(&package_dir)?;
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"esmpkg","version":"1.0.0","exports":{".":{"import":"./index.mjs"}}}"#,
        )?;
        std::fs::write(
            package_dir.join("index.mjs"),
            "export const val = 1;\nexport default 2;\n",
        )?;

        let result = transform_esm_to_cjs(
            "import dep, { val } from 'esmpkg';\nexport * from 'esmpkg';\nconst lazy = import('esmpkg');\nconsole.log(dep, val, lazy);\n",
            &root.join("app.mjs"),
        );
        assert!(result.is_transformed);
        assert!(
            !result.code.contains("require(\"esmpkg\")"),
            "no bare require of the esm-only package may remain: {}",
            result.code
        );
        assert!(
            result
                .code
                .contains("require(\"./node_modules/esmpkg/index.mjs\")"),
            "static, re-export, and dynamic imports should use the relative path: {}",
            result.code
        );
        assert!(
            !result.code.contains("import(\"esmpkg\")"),
            "dynamic import should be rewritten too: {}",
            result.code
        );

        std::fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn keeps_require_resolvable_bare_specifiers() -> Result<(), std::io::Error> {
        let root = std::env::temp_dir().join(format!("pkg-rust-esm-dual-{}", std::process::id()));
        let _ignored = std::fs::remove_dir_all(&root);
        let dual_dir = root.join("node_modules/dualpkg");
        std::fs::create_dir_all(&dual_dir)?;
        std::fs::write(
            dual_dir.join("package.json"),
            r#"{"name":"dualpkg","version":"1.0.0","exports":{".":{"require":"./index.cjs","import":"./index.mjs"}}}"#,
        )?;
        std::fs::write(dual_dir.join("index.cjs"), "module.exports = { val: 1 };\n")?;
        std::fs::write(dual_dir.join("index.mjs"), "export const val = 1;\n")?;
        let main_dir = root.join("node_modules/mainpkg");
        std::fs::create_dir_all(&main_dir)?;
        std::fs::write(
            main_dir.join("package.json"),
            r#"{"name":"mainpkg","version":"1.0.0","main":"index.js"}"#,
        )?;
        std::fs::write(main_dir.join("index.js"), "module.exports = { v: 2 };\n")?;
        let typemod_dir = root.join("node_modules/typemod");
        std::fs::create_dir_all(&typemod_dir)?;
        std::fs::write(
            typemod_dir.join("package.json"),
            r#"{"name":"typemod","version":"1.0.0","type":"module","exports":"./index.js"}"#,
        )?;
        std::fs::write(typemod_dir.join("index.js"), "export const t = 3;\n")?;

        let result = transform_esm_to_cjs(
            "import { val } from 'dualpkg';\nimport { v } from 'mainpkg';\nimport { t } from 'typemod';\nconsole.log(val, v, t);\n",
            &root.join("app.mjs"),
        );
        assert!(result.is_transformed);
        assert!(
            result.code.contains("require(\"dualpkg\")"),
            "dual packages stay require-resolvable and keep the bare form: {}",
            result.code
        );
        assert!(
            result.code.contains("require(\"mainpkg\")"),
            "classic main-resolved packages keep the bare form: {}",
            result.code
        );
        assert!(
            result.code.contains("require(\"typemod\")"),
            "`.js`-under-type-module exports targets snapshot in place and keep the bare form: {}",
            result.code
        );

        std::fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn rewrites_bare_imports_whose_require_target_is_mjs() -> Result<(), std::io::Error> {
        let root = std::env::temp_dir().join(format!("pkg-rust-esm-reqmjs-{}", std::process::id()));
        let _ignored = std::fs::remove_dir_all(&root);
        let package_dir = root.join("node_modules/reqesm");
        std::fs::create_dir_all(&package_dir)?;
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"reqesm","version":"1.0.0","exports":{".":{"require":"./index.mjs","import":"./index.mjs"}}}"#,
        )?;
        std::fs::write(package_dir.join("index.mjs"), "export const a = 1;\n")?;

        let result = transform_esm_to_cjs(
            "import { a } from 'reqesm';\nconsole.log(a);\n",
            &root.join("app.mjs"),
        );
        assert!(result.is_transformed);
        // The packer renames the transformed `.mjs` snapshot to `.js`, so a
        // bare require would resolve through exports to the missing `.mjs`.
        assert!(
            !result.code.contains("require(\"reqesm\")"),
            "require-condition `.mjs` targets must not stay bare: {}",
            result.code
        );
        assert!(
            result
                .code
                .contains("require(\"./node_modules/reqesm/index.mjs\")"),
            "the import should use the packaged file's relative path: {}",
            result.code
        );

        std::fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn rewrites_relative_mjs_require_paths() {
        let rewritten = rewrite_mjs_require_paths(
            "const a = require('./x.mjs'); const b = require(\"../y.mjs\"); const c = require('pkg/z.mjs');",
        );
        assert!(rewritten.contains("require('./x.js')"));
        assert!(rewritten.contains("require(\"../y.js\")"));
        assert!(rewritten.contains("require('pkg/z.mjs')"));
    }
}
