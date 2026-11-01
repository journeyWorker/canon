//! The pieces of the L1 checks that are not a check themselves: the typed documents of a `schema` answer, the
//! reading of a documented invocation, the rules of spec 10.13, field projection, the two regular-expression
//! engines, and the generation of the invocations that probe a declared constraint or domain.
//!
//! The checks themselves (`L1.tree` ... `L1.fields`) are in `checks/l1.rs`; they read the log of invocations the
//! run made, and these modules for everything that is a rule about a document.

pub mod invocation;
pub mod model;
pub mod project;
pub mod regex;
pub mod rules;
pub mod witness;
