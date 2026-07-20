//! Write-time diagnostics (design D3, task 3.2): the "expected …" shape
//! S12 D6 established for enum-domain mismatches, applied to CEL write-
//! time validation.

use canon_model::RecordKind;

use crate::registry::CelType;

/// A single write-time validation failure. Every variant names what was
/// expected — never a bare "invalid expression" (design's own acceptance
/// criterion: "with a diagnostic naming the expected type or the expected
/// member set").
#[derive(Debug, Clone, PartialEq)]
pub enum Diagnostic {
    /// `Program::compile` itself failed — a syntax error, not a binding
    /// mismatch.
    Syntax(String),
    /// A field select (`record.<field>`, including a `has(record.<field>)`
    /// presence test) named a field the target kind's schema does not
    /// declare.
    UndeclaredField { field: String, kind: RecordKind, expected: Vec<String> },
    /// A bare identifier other than `record` (or a macro-local binding)
    /// was referenced — canon-policy's closed profile exposes exactly one
    /// top-level variable.
    UndeclaredVariable { name: String, expected: Vec<String> },
    /// A `Call` node's function name is not in the allowlist (design D4)
    /// — neither a canon-policy-registered function nor a recognized CEL
    /// operator.
    UnknownFunction { name: String, expected: Vec<String> },
    /// An allowlisted function was called with the wrong number of
    /// arguments.
    ArityMismatch { function: String, expected: usize, got: usize },
    /// An allowlisted function was called with an argument whose
    /// statically-resolvable type does not match its declared signature.
    TypeMismatch { function: String, arg_index: usize, expected: CelType, got: CelType },
    /// A field select (`record.<field>` or a nested `<expr>.<field>`)
    /// was performed on an operand whose statically-known type is not a
    /// map/object — e.g. `record.title.foo` where `record.title` is
    /// `string`. Only emitted when the operand's type IS known and
    /// concrete; a genuinely unresolvable (`Dyn`) operand stays a
    /// runtime concern (review finding #1, ReviewS13 d26bc9d7).
    SelectOnNonObject { field: String, on_type: CelType },
    /// One of CEL's own binary arithmetic (`+ - * / %`) or ordering
    /// (`< <= > >=`) operators was applied to two operands whose
    /// statically-resolvable types are not a valid pair for that
    /// operator — e.g. comparing a string-backed enum field to an int,
    /// or adding an int field to a string. Equality (`==`/`!=`),
    /// membership (`@in`), and indexing stay runtime-deferred (review
    /// finding #2, ReviewS13 d26bc9d7 — see
    /// `crate::validate::check_call`'s doc for why); see
    /// [`Diagnostic::OperandTypeMismatch`] for the single-operand
    /// operators (`&&`/`||`/`!`, unary `-`, the ternary condition).
    OperatorTypeMismatch { operator: &'static str, left: CelType, right: CelType },
    /// A unary or short-circuit-boolean operator (`!`, unary `-`, `&&`,
    /// `||`, or a ternary's CONDITION operand) was applied to an operand
    /// whose statically-resolvable type does not match what that
    /// operator requires — e.g. `record.title && true` (`title` is
    /// `string`, `&&` requires `bool`). Companion to
    /// [`Diagnostic::OperatorTypeMismatch`] for operators that check ONE
    /// operand against a fixed expected class rather than two operands
    /// against each other (review follow-up on ReviewS13 d26bc9d7,
    /// found in the 90238dc0 re-review).
    OperandTypeMismatch { operator: &'static str, expected: &'static str, got: CelType },
    /// A compile-time complexity bound (node count, nesting depth,
    /// comprehension count, or comprehension nesting depth) was
    /// exceeded. `cel` 0.14 exposes no evaluation-time interrupt or
    /// step-limit hook (verified against its own source — see
    /// `crate::eval`'s module doc), so a structurally pathological
    /// expression is rejected here, before it can ever be stored or
    /// evaluated (review finding #3, ReviewS13 d26bc9d7).
    TooComplex { metric: &'static str, limit: usize, actual: usize },
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Diagnostic::Syntax(message) => write!(f, "{message}"),
            Diagnostic::UndeclaredField { field, kind, expected } => {
                write!(f, "`record.{field}` is not a declared field of `{}` (expected one of: {})", kind.as_str(), expected.join(", "))
            }
            Diagnostic::UndeclaredVariable { name, expected } => {
                write!(f, "`{name}` is not a declared variable (expected one of: {})", expected.join(", "))
            }
            Diagnostic::UnknownFunction { name, expected } => {
                write!(f, "`{name}` is not an allowlisted function (expected one of: {})", expected.join(", "))
            }
            Diagnostic::ArityMismatch { function, expected, got } => {
                write!(f, "`{function}` expects {expected} argument(s), got {got}")
            }
            Diagnostic::TypeMismatch { function, arg_index, expected, got } => {
                write!(f, "`{function}` expects argument {arg_index} of type `{expected}`, got `{got}`")
            }
            Diagnostic::SelectOnNonObject { field, on_type } => {
                write!(f, "cannot select `{field}` from a `{on_type}` value (expected a map/object type)")
            }
            Diagnostic::OperatorTypeMismatch { operator, left, right } => {
                write!(f, "`{operator}` requires compatible operand types, got `{left}` and `{right}`")
            }
            Diagnostic::OperandTypeMismatch { operator, expected, got } => {
                write!(f, "`{operator}` requires a {expected} operand, got `{got}`")
            }
            Diagnostic::TooComplex { metric, limit, actual } => {
                write!(f, "expression exceeds canon-policy's {metric} bound ({actual} > {limit})")
            }
        }
    }
}

impl std::error::Error for Diagnostic {}
