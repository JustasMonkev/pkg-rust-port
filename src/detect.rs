use swc_common::{FileName, SourceMap, sync::Lrc};
use swc_ecma_ast::{
    ArrayLit, BinExpr, Callee, Decl, Expr, ExprOrSpread, ImportDecl, ImportSpecifier, Lit,
    MemberExpr, MemberProp, ModuleDecl, ModuleExportName, ModuleItem, ObjectLit, Program, Prop,
    PropOrSpread, Stmt, VarDecl,
};
use swc_ecma_parser::{EsSyntax, Parser, StringInput, Syntax, lexer::Lexer};

use crate::common::AliasKind;
use crate::error::PkgError;

/// Resolved static dependency discovered in source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Derivative {
    /// Requested module/path alias.
    pub alias: String,
    /// How the alias should be resolved.
    pub alias_kind: AliasKind,
    /// Reconstructed debug line matching the JavaScript detector test mode.
    pub debug_line: String,
    /// Whether this dependency must be excluded.
    pub must_exclude: bool,
    /// Whether this dependency may be excluded.
    pub may_exclude: bool,
}

/// Source pattern detected by the JS detector port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DetectionKind {
    /// Literal `require`, `require.resolve`, `import`, or `path.join(__dirname, ...)`.
    Successful(Derivative),
    /// Dynamic `require` or `require.resolve` argument.
    NonLiteral {
        /// Reconstructed dynamic argument.
        alias: String,
        /// Whether this dependency must be excluded.
        must_exclude: bool,
        /// Whether this dependency may be excluded.
        may_exclude: bool,
    },
    /// A malformed `require` call with an argument.
    Malformed {
        /// Reconstructed argument.
        alias: String,
    },
    /// Ambiguous `path.resolve(...)` call.
    AmbiguousCwd {
        /// Reconstructed arguments.
        alias: String,
    },
}

/// One detected source use with its surrounding `try` context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectedUse {
    /// The detected pattern.
    pub kind: DetectionKind,
    /// Whether the pattern was nested inside a `try` statement.
    pub trying: bool,
}

/// Detect dependency-related source patterns.
///
/// # Example
///
/// ```
/// let uses = pkg_rust::detect(r#"require("fs"); path.join(__dirname, "view.html");"#)?;
/// assert_eq!(uses.len(), 2);
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn detect(source: &str) -> Result<Vec<DetectedUse>, PkgError> {
    let program = parse_program(source)?;
    let mut visitor = Detector::default();
    visitor.program(&program);
    Ok(visitor.detected)
}

/// Return the debug strings produced by `visitorSuccessful(node, true)`.
///
/// This exists to keep parity with `test-50-ast-parsing`, whose oracle checks
/// the detector's reconstructed static calls directly.
///
/// # Example
///
/// ```
/// let lines = pkg_rust::successful_debug_lines(r#"import app from "demo"; require("x");"#)?;
/// assert_eq!(lines, vec![r#"import app from "demo";"#, r#"require("x");"#]);
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn successful_debug_lines(source: &str) -> Result<Vec<String>, PkgError> {
    let program = parse_program(source)?;
    let mut visitor = Detector {
        include_invalid_successful: true,
        detected: Vec::new(),
    };
    visitor.program(&program);

    Ok(visitor
        .detected
        .into_iter()
        .filter_map(|detected| match detected.kind {
            DetectionKind::Successful(derivative) => Some(successful_debug_output(
                &successful_debug_line(&derivative),
                detected.trying,
            )),
            DetectionKind::NonLiteral { .. }
            | DetectionKind::Malformed { .. }
            | DetectionKind::AmbiguousCwd { .. } => None,
        })
        .collect())
}

/// Return debug strings for non-literal require and ambiguous cwd resolution.
///
/// # Example
///
/// ```
/// let lines = pkg_rust::non_literal_and_cwd_debug_lines(r#"require(name); path.resolve("a");"#)?;
/// assert_eq!(lines, vec!["name", "'a'"]);
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn non_literal_and_cwd_debug_lines(source: &str) -> Result<Vec<String>, PkgError> {
    Ok(detect(source)?
        .into_iter()
        .filter_map(|detected| match detected.kind {
            DetectionKind::NonLiteral { alias, .. } | DetectionKind::AmbiguousCwd { alias } => {
                Some(alias)
            }
            DetectionKind::Successful(_) | DetectionKind::Malformed { .. } => None,
        })
        .collect())
}

#[derive(Default)]
struct Detector {
    include_invalid_successful: bool,
    detected: Vec<DetectedUse>,
}

impl Detector {
    fn program(&mut self, program: &Program) {
        match program {
            Program::Module(module) => {
                for item in &module.body {
                    self.module_item(item, false);
                }
            }
            Program::Script(script) => {
                for stmt in &script.body {
                    self.stmt(stmt, false);
                }
            }
        }
    }

    fn module_item(&mut self, item: &ModuleItem, trying: bool) {
        match item {
            ModuleItem::ModuleDecl(ModuleDecl::Import(import)) => {
                self.import_decl(import, trying);
            }
            ModuleItem::ModuleDecl(_) => {}
            ModuleItem::Stmt(stmt) => self.stmt(stmt, trying),
        }
    }

    fn stmt(&mut self, stmt: &Stmt, trying: bool) {
        match stmt {
            Stmt::Block(block) => {
                for stmt in &block.stmts {
                    self.stmt(stmt, trying);
                }
            }
            Stmt::Return(return_stmt) => {
                if let Some(arg) = &return_stmt.arg {
                    self.expr(arg, trying);
                }
            }
            Stmt::Try(try_stmt) => {
                for stmt in &try_stmt.block.stmts {
                    self.stmt(stmt, true);
                }
                if let Some(handler) = &try_stmt.handler {
                    for stmt in &handler.body.stmts {
                        self.stmt(stmt, true);
                    }
                }
                if let Some(finalizer) = &try_stmt.finalizer {
                    for stmt in &finalizer.stmts {
                        self.stmt(stmt, true);
                    }
                }
            }
            Stmt::If(if_stmt) => {
                self.expr(&if_stmt.test, trying);
                self.stmt(&if_stmt.cons, trying);
                if let Some(alt) = &if_stmt.alt {
                    self.stmt(alt, trying);
                }
            }
            Stmt::Decl(decl) => self.decl(decl, trying),
            Stmt::Expr(expr_stmt) => self.expr(&expr_stmt.expr, trying),
            Stmt::Labeled(labeled) => self.stmt(&labeled.body, trying),
            Stmt::Throw(throw_stmt) => self.expr(&throw_stmt.arg, trying),
            Stmt::While(while_stmt) => {
                self.expr(&while_stmt.test, trying);
                self.stmt(&while_stmt.body, trying);
            }
            Stmt::For(for_stmt) => {
                if let Some(test) = &for_stmt.test {
                    self.expr(test, trying);
                }
                if let Some(update) = &for_stmt.update {
                    self.expr(update, trying);
                }
                self.stmt(&for_stmt.body, trying);
            }
            Stmt::Empty(_)
            | Stmt::Debugger(_)
            | Stmt::With(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Switch(_)
            | Stmt::DoWhile(_)
            | Stmt::ForIn(_)
            | Stmt::ForOf(_) => {}
        }
    }

    fn decl(&mut self, decl: &Decl, trying: bool) {
        match decl {
            Decl::Fn(function) => {
                if let Some(body) = &function.function.body {
                    for stmt in &body.stmts {
                        self.stmt(stmt, trying);
                    }
                }
            }
            Decl::Var(var) => self.var_decl(var, trying),
            Decl::Class(_)
            | Decl::Using(_)
            | Decl::TsInterface(_)
            | Decl::TsTypeAlias(_)
            | Decl::TsEnum(_)
            | Decl::TsModule(_) => {}
        }
    }

    fn var_decl(&mut self, decl: &VarDecl, trying: bool) {
        for declarator in &decl.decls {
            if let Some(init) = &declarator.init {
                self.expr(init, trying);
            }
        }
    }

    fn expr(&mut self, expr: &Expr, trying: bool) {
        if let Some(kind) = successful(expr, self.include_invalid_successful) {
            self.detected.push(DetectedUse { kind, trying });
            return;
        }

        if let Some(kind) = non_literal(expr) {
            self.detected.push(DetectedUse { kind, trying });
            return;
        }

        if let Some(kind) = malformed(expr) {
            self.detected.push(DetectedUse { kind, trying });
            return;
        }

        if let Some(kind) = ambiguous_cwd(expr) {
            self.detected.push(DetectedUse { kind, trying });
            return;
        }

        match expr {
            Expr::Call(call) => {
                if let Callee::Expr(callee) = &call.callee {
                    self.expr(callee, trying);
                }
                for arg in &call.args {
                    self.expr(&arg.expr, trying);
                }
            }
            Expr::Member(member) => {
                self.expr(&member.obj, trying);
                if let MemberProp::Computed(prop) = &member.prop {
                    self.expr(&prop.expr, trying);
                }
            }
            Expr::Bin(binary) => {
                self.expr(&binary.left, trying);
                self.expr(&binary.right, trying);
            }
            Expr::Cond(cond) => {
                self.expr(&cond.test, trying);
                self.expr(&cond.cons, trying);
                self.expr(&cond.alt, trying);
            }
            Expr::Array(array) => self.array_lit(array, trying),
            Expr::Object(object) => self.object_lit(object, trying),
            Expr::Fn(function) => {
                if let Some(body) = &function.function.body {
                    for stmt in &body.stmts {
                        self.stmt(stmt, trying);
                    }
                }
            }
            Expr::Paren(paren) => self.expr(&paren.expr, trying),
            Expr::Tpl(tpl) => {
                for expr in &tpl.exprs {
                    self.expr(expr, trying);
                }
            }
            Expr::This(_)
            | Expr::Unary(_)
            | Expr::Update(_)
            | Expr::Assign(_)
            | Expr::SuperProp(_)
            | Expr::Lit(_)
            | Expr::Ident(_)
            | Expr::New(_)
            | Expr::Seq(_)
            | Expr::TaggedTpl(_)
            | Expr::Arrow(_)
            | Expr::Class(_)
            | Expr::Yield(_)
            | Expr::MetaProp(_)
            | Expr::Await(_)
            | Expr::JSXMember(_)
            | Expr::JSXNamespacedName(_)
            | Expr::JSXEmpty(_)
            | Expr::JSXElement(_)
            | Expr::JSXFragment(_)
            | Expr::TsTypeAssertion(_)
            | Expr::TsConstAssertion(_)
            | Expr::TsNonNull(_)
            | Expr::TsAs(_)
            | Expr::TsInstantiation(_)
            | Expr::TsSatisfies(_)
            | Expr::PrivateName(_)
            | Expr::OptChain(_)
            | Expr::Invalid(_) => {}
        }
    }

    fn array_lit(&mut self, array: &ArrayLit, trying: bool) {
        for element in array.elems.iter().flatten() {
            self.expr(&element.expr, trying);
        }
    }

    fn object_lit(&mut self, object: &ObjectLit, trying: bool) {
        for prop in &object.props {
            match prop {
                PropOrSpread::Spread(spread) => self.expr(&spread.expr, trying),
                PropOrSpread::Prop(prop) => match prop.as_ref() {
                    Prop::KeyValue(key_value) => self.expr(&key_value.value, trying),
                    Prop::Assign(assign) => self.expr(&assign.value, trying),
                    Prop::Getter(getter) => {
                        if let Some(body) = &getter.body {
                            for stmt in &body.stmts {
                                self.stmt(stmt, trying);
                            }
                        }
                    }
                    Prop::Setter(setter) => {
                        if let Some(body) = &setter.body {
                            for stmt in &body.stmts {
                                self.stmt(stmt, trying);
                            }
                        }
                    }
                    Prop::Method(method) => {
                        if let Some(body) = &method.function.body {
                            for stmt in &body.stmts {
                                self.stmt(stmt, trying);
                            }
                        }
                    }
                    Prop::Shorthand(_) => {}
                },
            }
        }
    }

    fn import_decl(&mut self, import: &ImportDecl, trying: bool) {
        let alias = import.src.value.to_string_lossy().into_owned();
        let debug_line = reconstruct_import(import);
        self.detected.push(DetectedUse {
            kind: DetectionKind::Successful(Derivative {
                alias,
                alias_kind: AliasKind::Resolvable,
                debug_line,
                must_exclude: false,
                may_exclude: false,
            }),
            trying,
        });
    }
}

fn parse_program(source: &str) -> Result<Program, PkgError> {
    let cm: Lrc<SourceMap> = Default::default();
    let file = cm.new_source_file(
        FileName::Custom("input.js".into()).into(),
        source.to_owned(),
    );
    let lexer = Lexer::new(
        Syntax::Es(EsSyntax {
            jsx: true,
            ..Default::default()
        }),
        Default::default(),
        StringInput::from(&*file),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let program = parser
        .parse_program()
        .map_err(|error| PkgError::JavaScriptParse(format!("{error:?}")))?;
    if let Some(error) = parser.take_errors().into_iter().next() {
        return Err(PkgError::JavaScriptParse(format!("{error:?}")));
    }
    Ok(program)
}

fn successful(expr: &Expr, include_invalid_second: bool) -> Option<DetectionKind> {
    if let Some((alias, second)) = require_like(expr, RequireKind::Resolve) {
        if !include_invalid_second && !valid_second(second.as_deref()) {
            return None;
        }
        let debug_line = debug_require_line("require.resolve", &alias, second.as_deref());
        return Some(DetectionKind::Successful(Derivative {
            alias,
            alias_kind: AliasKind::Resolvable,
            debug_line,
            must_exclude: second.as_deref() == Some("must-exclude"),
            may_exclude: second.as_deref() == Some("may-exclude"),
        }));
    }

    if let Some((alias, second)) = require_like(expr, RequireKind::Plain) {
        if !include_invalid_second && !valid_second(second.as_deref()) {
            return None;
        }
        let debug_line = debug_require_line("require", &alias, second.as_deref());
        return Some(DetectionKind::Successful(Derivative {
            alias,
            alias_kind: AliasKind::Resolvable,
            debug_line,
            must_exclude: second.as_deref() == Some("must-exclude"),
            may_exclude: second.as_deref() == Some("may-exclude"),
        }));
    }

    if let Some(alias) = path_join_dirname(expr) {
        return Some(DetectionKind::Successful(Derivative {
            debug_line: format!(r#"path.join(__dirname, "{alias}")"#),
            alias,
            alias_kind: AliasKind::Relative,
            must_exclude: false,
            may_exclude: false,
        }));
    }

    None
}

fn non_literal(expr: &Expr) -> Option<DetectionKind> {
    let (arg, second) = non_literal_require_like(expr, RequireKind::Resolve)
        .or_else(|| non_literal_require_like(expr, RequireKind::Plain))?;
    if !valid_second(second.as_deref()) {
        return None;
    }
    Some(DetectionKind::NonLiteral {
        alias: reconstruct_expr(arg),
        must_exclude: second.as_deref() == Some("must-exclude"),
        may_exclude: second.as_deref() == Some("may-exclude"),
    })
}

fn malformed(expr: &Expr) -> Option<DetectionKind> {
    let arg = malformed_require_like(expr, RequireKind::Resolve)
        .or_else(|| malformed_require_like(expr, RequireKind::Plain))?;
    Some(DetectionKind::Malformed {
        alias: reconstruct_expr(arg),
    })
}

fn ambiguous_cwd(expr: &Expr) -> Option<DetectionKind> {
    let call = call_expr(expr)?;
    if !is_member_call(call, "path", "resolve") {
        return None;
    }
    Some(DetectionKind::AmbiguousCwd {
        alias: call
            .args
            .iter()
            .map(|arg| reconstruct_expr(&arg.expr))
            .collect::<Vec<_>>()
            .join(", "),
    })
}

fn require_like(expr: &Expr, kind: RequireKind) -> Option<(String, Option<String>)> {
    let call = call_expr(expr)?;
    if !matches_require_kind(call, kind) {
        return None;
    }
    let first = call.args.first()?;
    let alias = literal_value(&first.expr)?;
    let second = call.args.get(1).and_then(|arg| literal_value(&arg.expr));
    Some((alias, second))
}

fn non_literal_require_like(expr: &Expr, kind: RequireKind) -> Option<(&Expr, Option<String>)> {
    let call = call_expr(expr)?;
    if !matches_require_kind(call, kind) {
        return None;
    }
    let first = call.args.first()?;
    if literal_value(&first.expr).is_some() {
        return None;
    }
    let second = call.args.get(1).and_then(|arg| literal_value(&arg.expr));
    Some((&first.expr, second))
}

fn malformed_require_like(expr: &Expr, kind: RequireKind) -> Option<&Expr> {
    let call = call_expr(expr)?;
    if !matches_require_kind(call, kind) {
        return None;
    }
    call.args.first().map(|arg| arg.expr.as_ref())
}

fn path_join_dirname(expr: &Expr) -> Option<String> {
    let call = call_expr(expr)?;
    if !is_member_call(call, "path", "join") || call.args.len() != 2 {
        return None;
    }
    if !is_ident_expr(&call.args[0].expr, "__dirname") {
        return None;
    }
    literal_value(&call.args[1].expr)
}

fn call_expr(expr: &Expr) -> Option<&swc_ecma_ast::CallExpr> {
    if let Expr::Call(call) = expr {
        Some(call)
    } else {
        None
    }
}

#[derive(Clone, Copy)]
enum RequireKind {
    Plain,
    Resolve,
}

fn matches_require_kind(call: &swc_ecma_ast::CallExpr, kind: RequireKind) -> bool {
    match kind {
        RequireKind::Plain => matches!(
            &call.callee,
            Callee::Expr(callee) if is_ident_expr(callee, "require")
        ),
        RequireKind::Resolve => is_member_call(call, "require", "resolve"),
    }
}

fn is_member_call(call: &swc_ecma_ast::CallExpr, object: &str, property: &str) -> bool {
    matches!(
        &call.callee,
        Callee::Expr(callee)
            if matches!(
                callee.as_ref(),
                Expr::Member(member)
                    if is_ident_expr(&member.obj, object)
                        && matches!(&member.prop, MemberProp::Ident(ident) if ident.sym == property)
            )
    )
}

fn is_ident_expr(expr: &Expr, name: &str) -> bool {
    matches!(expr, Expr::Ident(ident) if ident.sym == name)
}

fn literal_value(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(Lit::Str(value)) => Some(value.value.to_string_lossy().into_owned()),
        Expr::Lit(Lit::Bool(value)) => Some(value.value.to_string()),
        Expr::Lit(Lit::Num(value)) => Some(number_lit(value)),
        Expr::Tpl(template) if template.exprs.is_empty() => template
            .quasis
            .first()
            .map(|element| element.raw.to_string()),
        _ => None,
    }
}

fn valid_second(second: Option<&str>) -> bool {
    matches!(second, None | Some("must-exclude") | Some("may-exclude"))
}

fn successful_debug_line(derivative: &Derivative) -> String {
    derivative.debug_line.clone()
}

fn successful_debug_output(line: &str, trying: bool) -> String {
    if trying {
        format!("try {{ {line}; }} catch (_) {{}}")
    } else {
        format!("{line};")
    }
}

fn debug_require_line(callee: &str, alias: &str, second: Option<&str>) -> String {
    if let Some(second) = second {
        format!(r#"{callee}("{alias}", "{second}")"#)
    } else {
        format!(r#"{callee}("{alias}")"#)
    }
}

fn reconstruct_import(import: &ImportDecl) -> String {
    let mut defaults = Vec::new();
    let mut named = Vec::new();

    for specifier in &import.specifiers {
        match specifier {
            ImportSpecifier::Default(default) => defaults.push(default.local.sym.to_string()),
            ImportSpecifier::Named(named_specifier) => {
                let imported = named_specifier
                    .imported
                    .as_ref()
                    .map(module_export_name)
                    .unwrap_or_else(|| named_specifier.local.sym.to_string());
                if named_specifier.local.sym == imported {
                    named.push(named_specifier.local.sym.to_string());
                } else {
                    named.push(format!("{imported} as {}", named_specifier.local.sym));
                }
            }
            ImportSpecifier::Namespace(namespace) => {
                defaults.push(format!("* as {}", namespace.local.sym))
            }
        }
    }

    if !named.is_empty() {
        defaults.push(format!("{{ {} }}", named.join(", ")));
    }

    format!(
        r#"import {} from "{}""#,
        defaults.join(", "),
        import.src.value.to_string_lossy()
    )
}

fn module_export_name(name: &ModuleExportName) -> String {
    match name {
        ModuleExportName::Ident(ident) => ident.sym.to_string(),
        ModuleExportName::Str(value) => value.value.to_string_lossy().into_owned(),
    }
}

fn reconstruct_expr(expr: &Expr) -> String {
    match expr {
        Expr::Ident(ident) => ident.sym.to_string(),
        Expr::Lit(Lit::Str(value)) => quote_single(value.value.to_string_lossy().as_ref()),
        Expr::Lit(Lit::Bool(value)) => value.value.to_string(),
        Expr::Lit(Lit::Num(value)) => number_lit(value),
        Expr::Lit(Lit::Null(_)) => "null".to_owned(),
        Expr::Member(member) => reconstruct_member(member),
        Expr::Call(call) => reconstruct_call(call),
        Expr::Bin(binary) => reconstruct_binary(binary),
        Expr::Cond(cond) => format!(
            "{} ? {} : {}",
            reconstruct_expr(&cond.test),
            reconstruct_expr(&cond.cons),
            reconstruct_expr(&cond.alt)
        ),
        Expr::Array(array) => reconstruct_array(array),
        Expr::Paren(paren) => reconstruct_expr(&paren.expr),
        Expr::Tpl(template) if template.exprs.is_empty() => template
            .quasis
            .first()
            .map(|element| format!("`{}`", element.raw))
            .unwrap_or_else(|| "``".to_owned()),
        _ => "<unknown>".to_owned(),
    }
}

fn reconstruct_member(member: &MemberExpr) -> String {
    let object = reconstruct_expr(&member.obj);
    match &member.prop {
        MemberProp::Ident(ident) => format!("{object}.{}", ident.sym),
        MemberProp::Computed(prop) => format!("{object}[{}]", reconstruct_expr(&prop.expr)),
        MemberProp::PrivateName(private) => format!("{object}.#{}", private.name),
    }
}

fn reconstruct_call(call: &swc_ecma_ast::CallExpr) -> String {
    let callee = match &call.callee {
        Callee::Expr(expr) => reconstruct_expr(expr),
        Callee::Super(_) => "super".to_owned(),
        Callee::Import(_) => "import".to_owned(),
    };
    let args = call
        .args
        .iter()
        .map(reconstruct_arg)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{callee}({args})")
}

fn reconstruct_arg(arg: &ExprOrSpread) -> String {
    if arg.spread.is_some() {
        format!("...{}", reconstruct_expr(&arg.expr))
    } else {
        reconstruct_expr(&arg.expr)
    }
}

fn reconstruct_binary(binary: &BinExpr) -> String {
    format!(
        "{} {} {}",
        reconstruct_expr(&binary.left),
        binary_op(binary.op),
        reconstruct_expr(&binary.right)
    )
}

fn reconstruct_array(array: &ArrayLit) -> String {
    let elems = array
        .elems
        .iter()
        .filter_map(|element| element.as_ref())
        .map(reconstruct_arg)
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{elems}]")
}

fn binary_op(op: swc_ecma_ast::BinaryOp) -> &'static str {
    match op {
        swc_ecma_ast::BinaryOp::Add => "+",
        swc_ecma_ast::BinaryOp::Sub => "-",
        swc_ecma_ast::BinaryOp::Mul => "*",
        swc_ecma_ast::BinaryOp::Div => "/",
        swc_ecma_ast::BinaryOp::Mod => "%",
        swc_ecma_ast::BinaryOp::EqEq => "==",
        swc_ecma_ast::BinaryOp::NotEq => "!=",
        swc_ecma_ast::BinaryOp::EqEqEq => "===",
        swc_ecma_ast::BinaryOp::NotEqEq => "!==",
        swc_ecma_ast::BinaryOp::Lt => "<",
        swc_ecma_ast::BinaryOp::LtEq => "<=",
        swc_ecma_ast::BinaryOp::Gt => ">",
        swc_ecma_ast::BinaryOp::GtEq => ">=",
        swc_ecma_ast::BinaryOp::LShift => "<<",
        swc_ecma_ast::BinaryOp::RShift => ">>",
        swc_ecma_ast::BinaryOp::ZeroFillRShift => ">>>",
        swc_ecma_ast::BinaryOp::BitOr => "|",
        swc_ecma_ast::BinaryOp::BitXor => "^",
        swc_ecma_ast::BinaryOp::BitAnd => "&",
        swc_ecma_ast::BinaryOp::LogicalOr => "||",
        swc_ecma_ast::BinaryOp::LogicalAnd => "&&",
        swc_ecma_ast::BinaryOp::In => "in",
        swc_ecma_ast::BinaryOp::InstanceOf => "instanceof",
        swc_ecma_ast::BinaryOp::Exp => "**",
        swc_ecma_ast::BinaryOp::NullishCoalescing => "??",
    }
}

fn number_lit(value: &swc_ecma_ast::Number) -> String {
    if let Some(raw) = &value.raw {
        return raw.to_string();
    }

    number_to_string(value.value)
}

fn number_to_string(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn quote_single(value: &str) -> String {
    format!("'{}'", value.replace('\'', "\\'"))
}
