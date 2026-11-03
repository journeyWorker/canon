//! The pieces of the L2 checks that are not a check themselves: the `--input` documents the checks build from an
//! invocation of the fixture (spec 11.5-11.7), the glob of `allowedEffects`, and the lines of a guide.
//!
//! The checks themselves (`L2.input-present` ... `L2.effects`) are in `checks/l2/`; they read the log of the invocations
//! the run made, and these modules for everything that is a rule about a document.

pub mod document;
pub mod glob;
pub mod guide;
