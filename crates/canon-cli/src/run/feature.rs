//! `canon feature` — the handlers of the group.

use canon_model::{ProjectId, Actor};
use chrono::{Timelike, Utc};
use crate::cli::feature::*;
use crate::scaffold::AreaSurface;
use std::process::ExitCode;

pub fn run(cmd: FeatureCommand) -> ExitCode {
    match cmd {
        FeatureCommand::New(FeatureNewArgs { surface, title, actor_id, project_id, repo, json, .. }) => run_feature_new(&repo, &surface, &title, project_id.as_ref(), &actor_id, json),
    }
}

/// `canon feature new <area>.<surface> --title <label> [--project-id
/// <id>]` (s16 P5, `crate::scaffold::run_feature_new`'s own doc)
/// — same single-`Utc::now()`-call discipline as `canon scenario new`.
/// `2` on a refused invocation (config fault, a spec root the
/// `--project-id` rules can't pin down, or an already-existing target
/// file), `0` on a fresh file
/// written.
fn run_feature_new(repo: &std::path::Path, surface: &AreaSurface, title: &str, project: Option<&ProjectId>, actor_id: &str, json: bool) -> ExitCode {
    let at = Utc::now().with_nanosecond(0).expect("0 is a valid nanosecond");
    let actor = Actor::new_unattributed(actor_id);
    ExitCode::from(crate::scaffold::run_feature_new(repo, surface, title, project, &actor, at, json) as u8)
}
