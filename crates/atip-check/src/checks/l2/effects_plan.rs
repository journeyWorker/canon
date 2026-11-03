//! The invocations of `L2.effects` (approaches/checker.md 4, 6): `--version`, `--help`, `--dry-run` and the fixture's successful
//! `read` commands, each run with its changes to the directory and `HOME` measured.
//!
//! The row runs at every level: from L1 with the command documents (every command's `--help`, every `--dry-run` of a listed
//! write, every listed `read`), at L0 with what needs none (`--version`, the root `--help`, the fixture's own `--dry-run` words).

use super::super::l1_plan::L1Run;
use super::super::{in_json_mode, Run};
use crate::exec::{Call, RunError};
use crate::l1::model::CommandDoc;

/// A measured invocation and what it was.
#[derive(Debug, Clone)]
pub(crate) struct EffectRun {
    pub label: String,
    pub call: usize,
}

/// A successful `read` command of the fixture, run again with its changes measured.
#[derive(Debug, Clone)]
pub(crate) struct ReadRun {
    pub command: String,
    pub call: usize,
    pub patterns: Vec<String>,
    /// Whether the command's document has a non-empty `effectNote`: only then may a change match `allowedEffects`.
    pub has_note: bool,
}

/// What `L2.effects` reads.
#[derive(Debug, Default)]
pub(crate) struct EffectsRun {
    pub version: Option<usize>,
    pub helps: Vec<EffectRun>,
    pub dry_runs: Vec<EffectRun>,
    pub reads: Vec<ReadRun>,
}

impl Run<'_> {
    pub(crate) fn plan_effects(&mut self, l1: Option<&L1Run>) -> Result<EffectsRun, RunError> {
        let version = self.invoke(Call::new(["--version"]).measured())?;
        let mut out = EffectsRun { version: Some(version), ..EffectsRun::default() };
        let call = self.invoke(Call::new(["--help"]).measured())?;
        out.helps.push(EffectRun { label: "--help".to_string(), call });
        let commands = self.fixture.commands.clone().unwrap_or_default();
        // The fixture's own `--dry-run` invocations, with or without documents.
        for (id, entry) in &commands {
            for argv in entry.ok.iter().chain(&entry.refused).filter(|argv| argv.iter().any(|word| word == "--dry-run")) {
                let call = self.invoke(Call::new(in_json_mode(argv)).measured())?;
                out.dry_runs.push(EffectRun { label: format!("{id}: the fixture's `{}`", argv.join(" ")), call });
            }
        }
        let Some(l1) = l1 else { return Ok(out) };
        let docs: Vec<CommandDoc> = l1.docs.commands.iter().map(|(id, doc)| CommandDoc { command: id.clone(), ..doc.clone() }).collect();
        for doc in &docs {
            if self.not_run(&doc.command) {
                continue;
            }
            // A write the fixture does not list is never run, and its `--help` is a call of it.
            if self.runnable(doc) {
                let mut argv = doc.path.clone();
                argv.push("--help".to_string());
                let call = self.invoke(Call::new(argv).measured())?;
                out.helps.push(EffectRun { label: format!("{} --help", doc.command), call });
            }
            let entry = commands.get(&doc.command);
            let listed: Vec<Vec<String>> = entry.map(|entry| entry.ok.clone()).unwrap_or_default();
            if doc.option("dry-run").is_some() {
                for argv in &listed {
                    if argv.iter().any(|word| word == "--dry-run") {
                        continue;
                    }
                    let mut dry = in_json_mode(argv);
                    dry.push("--dry-run".to_string());
                    let call = self.invoke(Call::new(dry).measured())?;
                    out.dry_runs.push(EffectRun { label: format!("{} --dry-run", doc.command), call });
                }
            }
            // What `schema` changes is `L1.no-state`'s and what `skills show` changes is `L2.skills`': one owner for one pass condition.
            if doc.effect == "read" && doc.command != "schema" && doc.command != "skills show" {
                let note = doc.raw.get("effectNote").and_then(serde_json::Value::as_str).is_some_and(|note| !note.trim().is_empty());
                for argv in &listed {
                    let call = self.invoke(Call::new(in_json_mode(argv)).measured())?;
                    out.reads.push(ReadRun {
                        command: doc.command.clone(),
                        call,
                        patterns: entry.map(|entry| entry.allowed_effects.clone()).unwrap_or_default(),
                        has_note: note,
                    });
                }
            }
        }
        Ok(out)
    }
}
