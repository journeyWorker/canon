//! Write-time validation (design D3, tasks.md group 3): `cel::Program` has
//! no built-in static type checker (`Program::compile` only catches
//! syntax; `Program::references()` returns flat top-level names — just
//! `record`, never the `record.<field>` paths a `Select` chain walks).
//! `canon-policy` therefore walks `Program::expression()`'s AST itself,
//! checking every `record.<field>` chain against the target kind's
//! [`BindingSet`] and every free function call against the allowlist,
//! before the expression is ever accepted (module doc for [`compile`]).
//!
//! # The closed CEL profile this validator enforces
//!
//! `canon-policy` evaluates against `cel::Context::empty()`
//! ([`crate::eval`]) — zero ambient built-ins, not even CEL's own
//! `size`/`contains`/`matches` standard functions — plus exactly one
//! registered function, `age_days`. This walker mirrors that closed
//! surface at write time: a free function call (`f(x)`, `target: None`)
//! must name `age_days` or an internal CEL operator (`==`, `&&`, `+`, …,
//! detected structurally — every internal operator's `func_name` contains
//! a character no CEL identifier can, e.g. `_==_`, `@in`); a method call
//! (`x.f()`, `target: Some(x)`) is never allowlisted, since `Context::
//! empty()` has no built-in methods to back it — rejecting it at write
//! time (rather than deferring to a runtime `UndeclaredReference` failure)
//! is exactly D3's stated preference.
//!
//! `has(record.field)` is CEL's own built-in macro (google/cel-spec), not
//! a `canon-policy`-registered function: the parser rewrites a
//! well-formed `has(...)` call into a field-presence `Select` (`test:
//! true`) before this walker ever sees a `Call` node for it — so
//! `has(record.severty)` is validated by the SAME field-select check as
//! `record.severty` itself, with the same diagnostic.
//!
//! # Beyond "declared field, allowlisted call" (review findings #1–#3,
//! ReviewS13 d26bc9d7)
//!
//! Three checks close gaps the initial walker left runtime-only:
//! - [`check_select`] rejects a field select off an operand whose
//!   statically-known type is NOT an object (`record.title.foo`, `title`
//!   being `string`) — only a genuinely unresolvable (`Dyn`) operand
//!   still defers to runtime.
//! - [`check_operator`] statically types CEL's own arithmetic/ordering
//!   operator grammar (`record.status > 5`, `record.schema + 'x'`)
//!   against the operand-class pairs each operator actually supports.
//! - [`check_complexity`] rejects a structurally pathological expression
//!   (deep nesting, a runaway comprehension chain) before it can ever be
//!   stored or evaluated — `cel` has no evaluation-time abort hook, so
//!   this is the write-time half of [`crate::eval`]'s eval-budget
//!   mitigation; see that module's doc for the runtime half.

use std::collections::{BTreeMap, BTreeSet};

use canon_model::RecordKind;
use cel::common::ast::{CallExpr, ComprehensionExpr, EntryExpr, Expr, IdedExpr};
use cel::{ParseErrors, Program};

use crate::bindings::BindingSet;
use crate::diagnostics::Diagnostic;
use crate::registry::CelType;

/// A CEL expression that has passed write-time syntax + type validation
/// (design D3) against a specific [`BindingSet`]. The only way to obtain
/// one is [`compile`]; [`crate::eval::evaluate`] takes a `&CompiledPolicy`,
/// never a raw source string, so an unvalidated expression can never
/// reach evaluation (design's own acceptance criterion: "never accepted
/// and left to fail on first evaluation"). Carries a snapshot of the
/// `BindingSet`'s `record_fields` it was validated against, so evaluation
/// (design D5/[`crate::eval`]) reconstructs the JSON→CEL value mapping
/// from the SAME resolved types the validator checked — never a second,
/// possibly-mismatched `BindingSet` the caller could pass by mistake.
#[derive(Debug, Clone)]
pub struct CompiledPolicy {
    pub(crate) source: String,
    pub(crate) kind: RecordKind,
    pub(crate) record_fields: BTreeMap<String, CelType>,
    /// The expression's max comprehension NESTING depth (`0` if it has
    /// no comprehension at all) — [`crate::eval::evaluate`]'s runtime
    /// companion to [`check_complexity`]'s compile-time bound: this
    /// number alone can't predict cost (a nested comprehension's
    /// RUNTIME iteration count depends on the record `evaluate` is
    /// eventually called with, not on anything visible here), but
    /// combined with that record's longest list at eval time it bounds
    /// the worst-case comprehension work `list_len ^ this` (review
    /// follow-up on ReviewS13 d26bc9d7, found in the 90238dc0
    /// re-review — a flat compile-time nesting cap and a flat runtime
    /// record-size cap, chosen independently, still permit their
    /// PRODUCT to be large; this field is what lets the two interact
    /// correctly instead).
    pub(crate) comprehension_nesting: usize,
}

impl CompiledPolicy {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn kind(&self) -> RecordKind {
        self.kind
    }
}

/// Parses `source`, then type-checks it against `bindings` (design D3).
/// `Program::compile` is wrapped in `catch_unwind` (design Risks, task
/// 4.2) as a defensive measure against `cel`'s own parser regardless of
/// upstream panic-safety claims — a caller submitting a malformed
/// expression gets `Err`, never a crash.
pub fn compile(source: &str, bindings: &BindingSet) -> Result<CompiledPolicy, Vec<Diagnostic>> {
    let compiled = std::panic::catch_unwind(|| Program::compile(source));
    let program = match compiled {
        Ok(Ok(program)) => program,
        Ok(Err(errors)) => return Err(vec![Diagnostic::Syntax(format_parse_errors(&errors))]),
        Err(_) => return Err(vec![Diagnostic::Syntax(format!("CEL parser panicked on input: {source:?}"))]),
    };

    let mut diagnostics = Vec::new();
    let complexity = measure_complexity(program.expression());
    push_complexity_diagnostics(&complexity, &mut diagnostics);
    let mut locals = BTreeSet::new();
    walk(program.expression(), bindings, &mut locals, &mut diagnostics);

    if diagnostics.is_empty() {
        Ok(CompiledPolicy {
            source: source.to_string(),
            kind: bindings.kind,
            record_fields: bindings.record_fields.clone(),
            comprehension_nesting: complexity.max_comprehension_nesting,
        })
    } else {
        Err(diagnostics)
    }
}

fn format_parse_errors(errors: &ParseErrors) -> String {
    format!("{errors}")
}

/// A CEL identifier never contains any of the punctuation internal
/// operator/macro `func_name`s are built from (`_==_`, `_&&_`, `@in`, …) —
/// used to tell "a real function/method call subject to the allowlist"
/// apart from "CEL's own operator grammar, always available, never
/// allowlist-checked" without depending on `cel`'s internal `operators`
/// module path.
fn is_identifier_like(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Recursively walks every node of the AST, collecting every diagnostic
/// found (not just the first) — a caller sees every mistake in one
/// rejection, not one-error-per-fix-and-resubmit cycle.
fn walk(node: &IdedExpr, bindings: &BindingSet, locals: &mut BTreeSet<String>, diagnostics: &mut Vec<Diagnostic>) {
    match &node.expr {
        Expr::Unspecified => {}
        Expr::Literal(_) => {}
        Expr::Ident(name) => check_ident(name, locals, diagnostics),
        Expr::Select(select) => {
            walk(&select.operand, bindings, locals, diagnostics);
            check_select(select, bindings, locals, diagnostics);
        }
        Expr::Call(call) => {
            if let Some(target) = &call.target {
                walk(target, bindings, locals, diagnostics);
            }
            for arg in &call.args {
                walk(arg, bindings, locals, diagnostics);
            }
            check_call(call, bindings, locals, diagnostics);
        }
        Expr::List(list) => {
            for element in &list.elements {
                walk(element, bindings, locals, diagnostics);
            }
        }
        Expr::Map(map) => {
            for entry in &map.entries {
                walk_entry(&entry.expr, bindings, locals, diagnostics);
            }
        }
        Expr::Struct(structure) => {
            for entry in &structure.entries {
                walk_entry(&entry.expr, bindings, locals, diagnostics);
            }
        }
        Expr::Comprehension(comprehension) => walk_comprehension(comprehension, bindings, locals, diagnostics),
    }
}

fn walk_entry(entry: &EntryExpr, bindings: &BindingSet, locals: &mut BTreeSet<String>, diagnostics: &mut Vec<Diagnostic>) {
    match entry {
        EntryExpr::MapEntry(map_entry) => {
            walk(&map_entry.key, bindings, locals, diagnostics);
            walk(&map_entry.value, bindings, locals, diagnostics);
        }
        EntryExpr::StructField(field) => walk(&field.value, bindings, locals, diagnostics),
    }
}

fn walk_comprehension(comprehension: &ComprehensionExpr, bindings: &BindingSet, locals: &mut BTreeSet<String>, diagnostics: &mut Vec<Diagnostic>) {
    // `iter_range`/`accu_init` are evaluated in the OUTER scope — the loop
    // variables are not bound yet.
    walk(&comprehension.iter_range, bindings, locals, diagnostics);
    walk(&comprehension.accu_init, bindings, locals, diagnostics);

    let mut added = Vec::new();
    for var in [Some(&comprehension.iter_var), comprehension.iter_var2.as_ref(), Some(&comprehension.accu_var)].into_iter().flatten() {
        if locals.insert(var.clone()) {
            added.push(var.clone());
        }
    }

    walk(&comprehension.loop_cond, bindings, locals, diagnostics);
    walk(&comprehension.loop_step, bindings, locals, diagnostics);
    walk(&comprehension.result, bindings, locals, diagnostics);

    for var in added {
        locals.remove(&var);
    }
}

fn check_ident(name: &str, locals: &BTreeSet<String>, diagnostics: &mut Vec<Diagnostic>) {
    if name == "record" || locals.contains(name) {
        return;
    }
    let mut expected: Vec<String> = vec!["record".to_string()];
    expected.extend(locals.iter().cloned());
    diagnostics.push(Diagnostic::UndeclaredVariable { name: name.to_string(), expected });
}

/// Resolves the statically-known [`CelType`] of an expression, following
/// `record.<field>` chains through [`BindingSet::record_fields`]. Returns
/// `None` when the type genuinely cannot be determined at write time (a
/// macro-local variable, a `Dyn` field, an arbitrary sub-expression) — in
/// which case the caller skips the check rather than guessing.
fn resolve_static_type(node: &IdedExpr, bindings: &BindingSet, locals: &BTreeSet<String>) -> Option<CelType> {
    match &node.expr {
        Expr::Literal(literal) => Some(literal_type(literal)),
        Expr::Ident(name) if name == "record" => Some(CelType::Map(bindings.record_fields.clone())),
        Expr::Ident(name) if locals.contains(name) => None,
        Expr::Select(select) => match resolve_static_type(&select.operand, bindings, locals)? {
            CelType::Map(fields) => fields.get(&select.field).cloned(),
            _ => None,
        },
        Expr::Call(call) if call.target.is_none() => bindings.function(&call.func_name).map(|sig| sig.returns.clone()),
        _ => None,
    }
}

fn literal_type(literal: &cel::common::ast::LiteralValue) -> CelType {
    use cel::common::ast::LiteralValue;
    match literal {
        LiteralValue::Boolean(_) => CelType::Bool,
        LiteralValue::Int(_) => CelType::Int,
        LiteralValue::UInt(_) => CelType::UInt,
        LiteralValue::Double(_) => CelType::Double,
        LiteralValue::String(_) => CelType::String,
        LiteralValue::Bytes(_) | LiteralValue::Null => CelType::Dyn,
    }
}

fn check_select(select: &cel::common::ast::SelectExpr, bindings: &BindingSet, locals: &BTreeSet<String>, diagnostics: &mut Vec<Diagnostic>) {
    let Some(operand_ty) = resolve_static_type(&select.operand, bindings, locals) else {
        // Operand type is genuinely unresolvable at write time (a
        // macro-local variable, an arbitrary sub-expression) — defer to
        // runtime, same as every other `None` from `resolve_static_type`.
        return;
    };
    let fields = match operand_ty {
        CelType::Map(fields) => fields,
        CelType::Dyn => return, // unconstrained shape — genuinely unknown, defer to runtime
        other => {
            // Review finding #1 (ReviewS13 d26bc9d7): the operand's type
            // IS known here and it is NOT an object — `record.title.foo`
            // where `title` is `string` can never resolve `foo` at
            // runtime either. Unlike the `Dyn` case above, there is no
            // ambiguity to defer: reject now, at write time.
            diagnostics.push(Diagnostic::SelectOnNonObject { field: select.field.clone(), on_type: other });
            return;
        }
    };
    if fields.contains_key(&select.field) {
        return;
    }
    let mut expected: Vec<String> = fields.keys().cloned().collect();
    expected.sort();
    diagnostics.push(Diagnostic::UndeclaredField { field: select.field.clone(), kind: bindings.kind, expected });
}

/// The coarse operand shape [`check_operator`] statically types CEL's
/// operator grammar against — deliberately coarser than [`CelType`]
/// equality: every numeric variant (`Int`/`UInt`/`Double`) is one
/// mutually-ordered class, and a plain string field is interchangeable
/// with a closed string enum for this purpose (the SAME equivalence
/// this crate's own accepted tests already rely on for `record.status
/// == 'completed'`).
#[derive(PartialEq, Eq, Clone, Copy)]
enum OperandClass {
    Numeric,
    Text,
    Timestamp,
    List,
    Bool,
    Map,
}

impl OperandClass {
    fn name(self) -> &'static str {
        match self {
            OperandClass::Numeric => "numeric",
            OperandClass::Text => "text",
            OperandClass::Timestamp => "timestamp",
            OperandClass::List => "list",
            OperandClass::Bool => "bool",
            OperandClass::Map => "map",
        }
    }
}

fn operand_class(ty: CelType) -> Option<OperandClass> {
    match ty {
        CelType::Int | CelType::UInt | CelType::Double => Some(OperandClass::Numeric),
        CelType::String | CelType::Enum(_) => Some(OperandClass::Text),
        CelType::Timestamp => Some(OperandClass::Timestamp),
        CelType::List(_) => Some(OperandClass::List),
        CelType::Bool => Some(OperandClass::Bool),
        CelType::Map(_) => Some(OperandClass::Map),
        // Genuinely unconstrained — could be anything at runtime, so
        // this is the one case that stays deferred (mirrors
        // `check_select`'s `Dyn` handling above).
        CelType::Dyn => None,
    }
}

/// Binary arithmetic (`+ - * / %`) / ordering (`< <= > >=`) operators —
/// the closed set of operand-CLASS PAIRS each actually supports at
/// runtime.
fn binary_operand_pair(func_name: &str) -> Option<(&'static str, &'static [OperandClass])> {
    use cel::common::ast::operators;
    if func_name == operators::ADD {
        Some(("+", &[OperandClass::Numeric, OperandClass::Text, OperandClass::List]))
    } else if func_name == operators::SUBSTRACT {
        Some(("-", &[OperandClass::Numeric]))
    } else if func_name == operators::MULTIPLY {
        Some(("*", &[OperandClass::Numeric]))
    } else if func_name == operators::DIVIDE {
        Some(("/", &[OperandClass::Numeric]))
    } else if func_name == operators::MODULO {
        Some(("%", &[OperandClass::Numeric]))
    } else if func_name == operators::LESS {
        Some(("<", &[OperandClass::Numeric, OperandClass::Text, OperandClass::Timestamp]))
    } else if func_name == operators::LESS_EQUALS {
        Some(("<=", &[OperandClass::Numeric, OperandClass::Text, OperandClass::Timestamp]))
    } else if func_name == operators::GREATER {
        Some((">", &[OperandClass::Numeric, OperandClass::Text, OperandClass::Timestamp]))
    } else if func_name == operators::GREATER_EQUALS {
        Some((">=", &[OperandClass::Numeric, OperandClass::Text, OperandClass::Timestamp]))
    } else {
        None
    }
}

/// Single-operand-class operators: `&&`/`||` (both operands must be
/// `bool`), unary `!` (its one operand must be `bool`), unary `-` (its
/// one operand must be numeric), and the ternary's CONDITION operand
/// (index 0 only — the two branches are never type-unified by CEL
/// itself). Returns the argument INDICES this operator requires to be
/// `expected`, e.g. `&&`/`||` check indices `[0, 1]`, the ternary checks
/// only `[0]`.
fn single_class_operand_indices(func_name: &str) -> Option<(&'static str, OperandClass, &'static [usize])> {
    use cel::common::ast::operators;
    if func_name == operators::LOGICAL_AND {
        Some(("&&", OperandClass::Bool, &[0, 1]))
    } else if func_name == operators::LOGICAL_OR {
        Some(("||", OperandClass::Bool, &[0, 1]))
    } else if func_name == operators::LOGICAL_NOT {
        Some(("!", OperandClass::Bool, &[0]))
    } else if func_name == operators::NEGATE {
        Some(("-", OperandClass::Numeric, &[0]))
    } else if func_name == operators::CONDITIONAL {
        Some(("?:", OperandClass::Bool, &[0]))
    } else {
        None
    }
}

/// Statically types CEL's own operator grammar against the operand-
/// class shape each operator actually supports at runtime (review
/// finding #2, ReviewS13 d26bc9d7, extended per the 90238dc0 re-review's
/// follow-up finding): `record.status > 5`, `record.schema + 'x'`, and
/// `record.title && true` all used to parse clean and fail only at
/// evaluation. Equality (`==`/`!=`), membership (`@in`), and indexing
/// stay runtime-deferred: equality is intentionally permissive across
/// related types (an enum field compared to a string literal, this
/// crate's own `record.status == 'completed'` test), and `@in`/indexing
/// operand types are rarely statically resolvable in the shapes this
/// crate's bindings produce. Every operand actually checked here (either
/// branch below) is skipped, not rejected, whenever its type genuinely
/// can't be resolved (a macro-local variable, another operator call's
/// result — `resolve_static_type` deliberately does not attempt to
/// infer THOSE) or resolves to `Dyn`.
fn check_operator(func_name: &str, call: &CallExpr, bindings: &BindingSet, locals: &BTreeSet<String>, diagnostics: &mut Vec<Diagnostic>) {
    if let Some((symbol, allowed)) = binary_operand_pair(func_name) {
        let [left, right] = call.args.as_slice() else {
            // The parser only ever builds these operator nodes with
            // exactly two arguments; a different arity would be a `cel`
            // parser change this crate's `dependency_audit` version pin
            // exists to catch, not user-rejectable input.
            return;
        };
        let Some(left_ty) = resolve_static_type(left, bindings, locals) else { return };
        let Some(right_ty) = resolve_static_type(right, bindings, locals) else { return };
        let (Some(left_class), Some(right_class)) = (operand_class(left_ty.clone()), operand_class(right_ty.clone())) else {
            return;
        };
        if left_class != right_class || !allowed.contains(&left_class) {
            diagnostics.push(Diagnostic::OperatorTypeMismatch { operator: symbol, left: left_ty, right: right_ty });
        }
        return;
    }

    if let Some((symbol, expected, indices)) = single_class_operand_indices(func_name) {
        for &index in indices {
            let Some(arg) = call.args.get(index) else { continue };
            let Some(ty) = resolve_static_type(arg, bindings, locals) else { continue };
            let Some(class) = operand_class(ty.clone()) else { continue };
            if class != expected {
                diagnostics.push(Diagnostic::OperandTypeMismatch { operator: symbol, expected: expected.name(), got: ty });
            }
        }
    }
}

fn check_call(call: &CallExpr, bindings: &BindingSet, locals: &BTreeSet<String>, diagnostics: &mut Vec<Diagnostic>) {
    if !is_identifier_like(&call.func_name) {
        // CEL's own operator grammar (`_==_`, `_&&_`, `@in`, …) — always
        // available, never allowlist-checked. A closed subset gets a
        // write-time operand-type check; see `check_operator`'s doc.
        check_operator(&call.func_name, call, bindings, locals, diagnostics);
        return;
    }
    if call.target.is_some() {
        // Method-call syntax: `Context::empty()` (design's closed
        // profile, this module's doc) has no built-in methods to back it.
        let expected = bindings.callable_function_names();
        diagnostics.push(Diagnostic::UnknownFunction { name: call.func_name.clone(), expected });
        return;
    }
    let Some(sig) = bindings.function(&call.func_name) else {
        let expected = bindings.callable_function_names();
        diagnostics.push(Diagnostic::UnknownFunction { name: call.func_name.clone(), expected });
        return;
    };
    if call.args.len() != sig.args.len() {
        diagnostics.push(Diagnostic::ArityMismatch { function: sig.name.to_string(), expected: sig.args.len(), got: call.args.len() });
        return;
    }
    for (index, (arg, expected_ty)) in call.args.iter().zip(&sig.args).enumerate() {
        if *expected_ty == CelType::Dyn {
            continue;
        }
        if let Some(actual) = resolve_static_type(arg, bindings, locals) {
            if actual != CelType::Dyn && &actual != expected_ty {
                diagnostics.push(Diagnostic::TypeMismatch { function: sig.name.to_string(), arg_index: index, expected: expected_ty.clone(), got: actual });
            }
        }
    }
}

/// Compile-time complexity bounds (review finding #3, ReviewS13
/// d26bc9d7 — see [`crate::eval`]'s module doc for the full rationale):
/// `cel` 0.14 has no evaluation-time interrupt or step-limit hook, so a
/// structurally pathological expression cannot be stopped mid-
/// evaluation from outside the interpreter. These bounds are generous
/// relative to every real expression in this crate's own fixtures/tests
/// (the longest, three CHAINED — not nested — `.map()` calls, sits at 3
/// comprehensions and comprehension-nesting depth 1) while still
/// catching genuinely pathological input (deep nesting, runaway
/// comprehension chains) before it can ever reach evaluation.
const MAX_AST_NODES: usize = 300;
const MAX_AST_DEPTH: usize = 40;
const MAX_COMPREHENSIONS: usize = 8;
const MAX_COMPREHENSION_NESTING: usize = 3;

#[derive(Default)]
struct Complexity {
    nodes: usize,
    max_depth: usize,
    comprehensions: usize,
    max_comprehension_nesting: usize,
}

fn measure_complexity(node: &IdedExpr) -> Complexity {
    let mut out = Complexity::default();
    measure(node, 0, 0, &mut out);
    out
}

fn measure(node: &IdedExpr, depth: usize, comprehension_depth: usize, out: &mut Complexity) {
    out.nodes += 1;
    out.max_depth = out.max_depth.max(depth);
    match &node.expr {
        Expr::Unspecified | Expr::Literal(_) | Expr::Ident(_) => {}
        Expr::Select(select) => measure(&select.operand, depth + 1, comprehension_depth, out),
        Expr::Call(call) => {
            if let Some(target) = &call.target {
                measure(target, depth + 1, comprehension_depth, out);
            }
            for arg in &call.args {
                measure(arg, depth + 1, comprehension_depth, out);
            }
        }
        Expr::List(list) => {
            for element in &list.elements {
                measure(element, depth + 1, comprehension_depth, out);
            }
        }
        Expr::Map(map) => {
            for entry in &map.entries {
                measure_entry(&entry.expr, depth + 1, comprehension_depth, out);
            }
        }
        Expr::Struct(structure) => {
            for entry in &structure.entries {
                measure_entry(&entry.expr, depth + 1, comprehension_depth, out);
            }
        }
        Expr::Comprehension(comprehension) => {
            out.comprehensions += 1;
            let nested_depth = comprehension_depth + 1;
            out.max_comprehension_nesting = out.max_comprehension_nesting.max(nested_depth);
            // `iter_range`/`accu_init` run ONCE, in the OUTER scope,
            // before this comprehension's own loop body starts (same
            // distinction `walk_comprehension` above documents) — a
            // comprehension CHAIN (`.map(...).map(...).map(...)`) is
            // additive cost, reached only through here, so it does NOT
            // count toward nesting depth. Only `loop_cond`/`loop_step`/
            // `result` run PER ITERATION — nesting a comprehension
            // inside one of THOSE is the multiplicative-cost dimension
            // this bound exists to catch.
            measure(&comprehension.iter_range, depth + 1, comprehension_depth, out);
            measure(&comprehension.accu_init, depth + 1, comprehension_depth, out);
            measure(&comprehension.loop_cond, depth + 1, nested_depth, out);
            measure(&comprehension.loop_step, depth + 1, nested_depth, out);
            measure(&comprehension.result, depth + 1, nested_depth, out);
        }
    }
}

fn measure_entry(entry: &EntryExpr, depth: usize, comprehension_depth: usize, out: &mut Complexity) {
    match entry {
        EntryExpr::MapEntry(map_entry) => {
            measure(&map_entry.key, depth, comprehension_depth, out);
            measure(&map_entry.value, depth, comprehension_depth, out);
        }
        EntryExpr::StructField(field) => measure(&field.value, depth, comprehension_depth, out),
    }
}

fn push_complexity_diagnostics(c: &Complexity, diagnostics: &mut Vec<Diagnostic>) {
    let bounds: [(&'static str, usize, usize); 4] = [
        ("node count", MAX_AST_NODES, c.nodes),
        ("nesting depth", MAX_AST_DEPTH, c.max_depth),
        ("comprehension count", MAX_COMPREHENSIONS, c.comprehensions),
        ("comprehension nesting depth", MAX_COMPREHENSION_NESTING, c.max_comprehension_nesting),
    ];
    for (metric, limit, actual) in bounds {
        if actual > limit {
            diagnostics.push(Diagnostic::TooComplex { metric, limit, actual });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::bindings_for;
    use crate::registry::SchemaRegistry;

    fn task_bindings() -> BindingSet {
        bindings_for(RecordKind::Task, &SchemaRegistry::load())
    }

    #[test]
    fn accepts_a_valid_field_comparison() {
        let bindings = task_bindings();
        assert!(compile("record.title == 'x'", &bindings).is_ok());
    }

    #[test]
    fn rejects_undeclared_field_with_expected_list() {
        let bindings = task_bindings();
        let errors = compile("record.severty == 'x'", &bindings).unwrap_err();
        assert_eq!(errors.len(), 1);
        let Diagnostic::UndeclaredField { field, expected, .. } = &errors[0] else {
            panic!("expected UndeclaredField, got {:?}", errors[0]);
        };
        assert_eq!(field, "severty");
        assert!(expected.contains(&"title".to_string()));
    }

    #[test]
    fn rejects_wrong_arity() {
        let bindings = task_bindings();
        let errors = compile("age_days()", &bindings).unwrap_err();
        assert!(matches!(errors[0], Diagnostic::ArityMismatch { expected: 1, got: 0, .. }));
    }

    #[test]
    fn rejects_wrong_argument_type() {
        let bindings = task_bindings();
        let errors = compile("age_days(record.title)", &bindings).unwrap_err();
        assert!(matches!(&errors[0], Diagnostic::TypeMismatch { expected: CelType::Timestamp, got: CelType::String, .. }));
    }

    #[test]
    fn accepts_age_days_on_a_timestamp_field() {
        let bindings = task_bindings();
        assert!(compile("age_days(record.at) > 30", &bindings).is_ok());
    }

    #[test]
    fn rejects_unknown_function() {
        let bindings = task_bindings();
        let errors = compile("mystery(record.title)", &bindings).unwrap_err();
        assert!(matches!(&errors[0], Diagnostic::UnknownFunction { name, .. } if name == "mystery"));
    }

    #[test]
    fn rejects_method_call_syntax() {
        let bindings = task_bindings();
        let errors = compile("record.title.contains('x')", &bindings).unwrap_err();
        assert!(matches!(&errors[0], Diagnostic::UnknownFunction { name, .. } if name == "contains"));
    }

    #[test]
    fn rejects_bare_undeclared_variable() {
        let bindings = task_bindings();
        let errors = compile("mystery_var == 1", &bindings).unwrap_err();
        assert!(matches!(&errors[0], Diagnostic::UndeclaredVariable { name, .. } if name == "mystery_var"));
    }

    #[test]
    fn has_macro_validates_the_underlying_field() {
        let bindings = task_bindings();
        assert!(compile("has(record.title)", &bindings).is_ok());
        let errors = compile("has(record.severty)", &bindings).unwrap_err();
        assert!(matches!(&errors[0], Diagnostic::UndeclaredField { field, .. } if field == "severty"));
    }

    #[test]
    fn accepts_map_macro_with_local_iter_var() {
        let bindings = task_bindings();
        // `title` is a string; `.map` over a string isn't meaningful CEL,
        // but this only exercises that `x` (the macro-local var) doesn't
        // trip the undeclared-variable check.
        assert!(compile("[1, 2, 3].map(x, x * 2) != []", &bindings).is_ok());
    }

    #[test]
    fn rejects_syntax_errors() {
        let bindings = task_bindings();
        let errors = compile("record.title ==", &bindings).unwrap_err();
        assert!(matches!(&errors[0], Diagnostic::Syntax(_)));
    }

    #[test]
    fn rejects_select_on_a_known_non_object_type() {
        // Review finding #1 (ReviewS13 d26bc9d7): `title` is `string`,
        // not a map — `.foo` can never resolve, at write time OR runtime.
        let bindings = task_bindings();
        let errors = compile("record.title.foo == 'x'", &bindings).unwrap_err();
        assert!(matches!(&errors[0], Diagnostic::SelectOnNonObject { field, on_type: CelType::String } if field == "foo"));
    }

    #[test]
    fn rejects_select_on_a_list_field() {
        let bindings = bindings_for(RecordKind::Handoff, &SchemaRegistry::load());
        let errors = compile("record.tags.foo", &bindings).unwrap_err();
        assert!(matches!(&errors[0], Diagnostic::SelectOnNonObject { field, on_type: CelType::List(_) } if field == "foo"));
    }

    #[test]
    fn select_on_a_dyn_field_stays_runtime_deferred() {
        // `body.fields` (Handoff) resolves to `Dyn` (open JSON) — its
        // shape is genuinely unknown at write time, so a further select
        // off it must NOT be write-time rejected (contrast with the two
        // tests above, whose operand types ARE known and concrete).
        let bindings = bindings_for(RecordKind::Handoff, &SchemaRegistry::load());
        assert!(compile("record.body.fields.anything == 'x'", &bindings).is_ok());
    }

    #[test]
    fn rejects_comparison_between_incompatible_operand_types() {
        // Review finding #2 (ReviewS13 d26bc9d7): `status` is a string-
        // backed enum; comparing it to an int used to parse clean and
        // fail only at evaluation.
        let bindings = task_bindings();
        let errors = compile("record.status > 5", &bindings).unwrap_err();
        assert!(matches!(&errors[0], Diagnostic::OperatorTypeMismatch { operator, .. } if *operator == ">"));
    }

    #[test]
    fn rejects_arithmetic_between_incompatible_operand_types() {
        // Review finding #2's other reviewer example: `schema` is `uint`,
        // added to a string literal.
        let bindings = task_bindings();
        let errors = compile("record.schema + 'x' == 'x'", &bindings).unwrap_err();
        assert!(matches!(&errors[0], Diagnostic::OperatorTypeMismatch { operator, .. } if *operator == "+"));
    }

    #[test]
    fn accepts_string_concatenation_and_numeric_comparison() {
        // Same operators, VALID operand pairs — proves `check_operator`
        // doesn't over-reject: string + string (`+`), and uint < uint.
        let bindings = task_bindings();
        assert!(compile("record.title + 'x' == 'x'", &bindings).is_ok());
        assert!(compile("record.schema < 5", &bindings).is_ok());
    }

    #[test]
    fn rejects_pathologically_nested_comprehensions() {
        // Review finding #3 (ReviewS13 d26bc9d7): `cel` has no
        // evaluation-time abort hook, so a structurally pathological
        // expression (here, 4-deep nested `.map()` — each level's
        // transform is itself a full `.map()` over a fresh list, the
        // genuinely multiplicative-cost shape, not a flat chain) is
        // rejected at write time instead — it must never reach
        // `crate::eval::evaluate` at all.
        let bindings = task_bindings();
        let pathological = "[1,2,3].map(a, [1,2,3].map(b, [1,2,3].map(c, [1,2,3].map(d, a + b + c + d)))) != []";
        let errors = compile(pathological, &bindings).unwrap_err();
        assert!(
            errors.iter().any(|d| matches!(d, Diagnostic::TooComplex { metric, .. } if *metric == "comprehension nesting depth")),
            "expected a comprehension-nesting-depth TooComplex diagnostic, got {errors:?}"
        );
    }

    #[test]
    fn a_comprehension_chain_is_not_penalized_as_nested() {
        // Sequential (chained) `.map()` calls are additive cost, not
        // multiplicative — must stay accepted even though this crate's
        // OWN eval budget test (`crate::eval::tests::
        // budget_exceeded_never_blocks_the_caller`) uses exactly this
        // shape.
        let bindings = task_bindings();
        assert!(compile("[1,2,3,4,5,6,7,8,9,10].map(x, x * 2).map(x, x * 2).map(x, x * 2) != []", &bindings).is_ok());
    }

    #[test]
    fn rejects_logical_and_or_on_a_non_bool_known_operand() {
        // Re-review follow-up on ReviewS13 d26bc9d7 (90238dc0): `title`
        // is a `string` field, directly select-resolvable — `&&`/`||`
        // require `bool` on BOTH sides, and this must reject at write
        // time, not defer to runtime.
        let bindings = task_bindings();
        let and_errors = compile("record.title && true", &bindings).unwrap_err();
        assert!(matches!(&and_errors[0], Diagnostic::OperandTypeMismatch { operator, expected, .. } if *operator == "&&" && *expected == "bool"));
        let or_errors = compile("true || record.title", &bindings).unwrap_err();
        assert!(matches!(&or_errors[0], Diagnostic::OperandTypeMismatch { operator, .. } if *operator == "||"));
    }

    #[test]
    fn rejects_logical_not_and_unary_negate_on_a_known_wrong_type() {
        let bindings = task_bindings();
        let not_errors = compile("!record.title == 'x'", &bindings).unwrap_err();
        assert!(matches!(&not_errors[0], Diagnostic::OperandTypeMismatch { operator, expected, .. } if *operator == "!" && *expected == "bool"));
        let negate_errors = compile("-record.title == 'x'", &bindings).unwrap_err();
        assert!(matches!(&negate_errors[0], Diagnostic::OperandTypeMismatch { operator, expected, .. } if *operator == "-" && *expected == "numeric"));
    }

    #[test]
    fn rejects_a_non_bool_ternary_condition() {
        let bindings = task_bindings();
        let errors = compile("record.title ? 'a' : 'b'", &bindings).unwrap_err();
        assert!(matches!(&errors[0], Diagnostic::OperandTypeMismatch { operator, expected, .. } if *operator == "?:" && *expected == "bool"));
    }

    #[test]
    fn accepts_boolean_ternary_and_negate_operators_on_correctly_typed_operands() {
        // Same operators, VALID operand shapes — proves the new checks
        // don't over-reject.
        let bindings = task_bindings();
        assert!(compile("record.status == 'done' && !(record.title == 'x')", &bindings).is_ok());
        assert!(compile("(record.schema > 0) ? record.title : 'default'", &bindings).is_ok());
        assert!(compile("-1 == record.schema", &bindings).is_ok());
    }

    #[test]
    fn boolean_operators_stay_deferred_on_a_nested_operator_operand() {
        // A boolean built from ANOTHER operator call's result (as
        // opposed to a direct field select) is not statically
        // resolvable — `resolve_static_type` deliberately does not
        // infer operator-call return types, so this must NOT be
        // rejected (mirrors `check_operator`'s own doc rationale).
        let bindings = task_bindings();
        assert!(compile("record.status == 'done' && age_days(record.at) > 7", &bindings).is_ok());
    }
}
