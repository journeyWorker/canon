//! The L2 checks of approaches/checker.md 6: JSON input and batches, destructive safety and effects, streams and guides.
//!
//! The invocations are made by the `*_plan` modules; each check reads the log and the documents the L1 run read from
//! `schema`. **One owner per pass condition**, as at L0 and L1.
//!
//! - [`input`]: the 15 rows of `--input` and batches (`L2.input-present` ... `L2.batch-ok`).

mod effects_plan;
mod guides;
mod guides_plan;
mod input;
mod input_plan;
mod safety;
mod safety_plan;

use super::l1_plan::L1Run;
use super::{Entry, Finding, Invocations, Judge, Run};
use crate::claim::Claim;
use crate::exec::RunError;
pub(crate) use effects_plan::EffectsRun;
use input_plan::InputRun;
use guides_plan::GuidesRun;
use safety_plan::SafetyRun;

/// What the L2 judgments read besides the log.
#[derive(Debug, Default)]
pub(crate) struct L2Run {
    pub input: InputRun,
    pub safety: SafetyRun,
    pub guides: GuidesRun,
}

/// The L2 checks of this run, in the order of approaches/checker.md.
pub(crate) fn table() -> impl Iterator<Item = &'static Entry> {
    input::TABLE.iter().chain(safety::TABLE.iter()).chain(guides::TABLE.iter()).chain(std::iter::once(&safety::EFFECTS))
}

/// `L2.effects` alone: the row an L0 or L1 run reports as an advisory.
pub(crate) fn effects_entry() -> &'static Entry {
    &safety::EFFECTS
}

/// The id of the effects row.
pub(crate) const EFFECTS_ID: &str = safety::EFFECTS_ID;

/// The write commands with `input` that no fixture entry lists, named by the rows that would have run them.
pub(crate) fn unlisted(id: &str, run: &L2Run) -> Vec<String> {
    if id.starts_with("L2.input-") && id != "L2.input-present" && id != "L2.input-example" || id.starts_with("L2.batch-") {
        run.input.unlisted.clone()
    } else if id == "L2.idempotency" {
        run.safety.unlisted_writes.clone()
    } else {
        Vec::new()
    }
}

impl Run<'_> {
    /// The invocations of the L2 checks, after the L0 and L1 ones: `l1` is what the L1 run read from `schema`.
    pub(crate) fn invoke_l2(&mut self, calls: &Invocations, l1: Option<&L1Run>, claim: Claim) -> Result<L2Run, RunError> {
        let mut run = L2Run::default();
        if let Some(l1) = l1 {
            run.input = self.plan_input(calls, l1, claim)?;
            run.safety = self.plan_safety(calls, l1)?;
            run.guides = self.plan_guides(calls, l1)?;
        }
        Ok(run)
    }
}

impl<'a> Judge<'a> {
    pub(crate) fn l2(&self) -> &'a L2Run {
        self.l2.expect("the L2 checks run only with an L2 run")
    }

    /// The findings of the L2 checks, in table order.
    pub(crate) fn l2_findings(&'a self) -> Vec<Finding> {
        let mut findings = self.input_findings();
        findings.extend(self.safety_findings());
        findings.extend(self.guides_findings());
        findings.push(self.l2_effects(self.effects));
        findings
    }
}
