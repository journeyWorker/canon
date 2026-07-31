# s40 plan-vs-actual-diff — tasks

## 1. Bind a dispatched run to its plan task

- [ ] 1.1 `canon dispatch begin` gains `--task <task_id>` and
      `--parent-run <run_id>`, both optional, populating `Run.task_id`
      and `Run.parent_run_id` instead of the hardcoded `None, None`.
- [ ] 1.2 `--task` is validated against the plan corpus: an unknown
      task_id fails loud with a nonzero exit and names the id, rather
      than persisting a dangling binding. Depends on 1.1.
- [ ] 1.3 The written manifest round-trips both fields, and a dispatch
      with neither flag is byte-identical to today's manifest.
      Depends on 1.1.
- [ ] 1.4 `canon retrieve` accepts `-k` as well as `--k`. Unrelated to
      the binding; grouped here because it is the same clap surface.

## 2. canon dispatch diff

- [ ] 2.1 Build the DECLARED edge set from `Task.depends_on` and the
      OBSERVED edge set from runs carrying both `task_id` and
      `parent_run_id` (an observed edge is parent-run's task → child
      run's task). Runs missing either field contribute nothing.
- [ ] 2.2 Classify every edge: satisfied (declared and observed),
      declared-not-observed, and observed-not-declared. Deterministic
      ordering. Depends on 2.1.
- [ ] 2.3 `canon dispatch diff [--repo] [--json]` renders the three
      classes. ALWAYS exits `0` on a successful read — it reports, never
      gates. An empty corpus reports zero edges, not an error.
      Depends on 2.2.

## 3. Make the funnel a funnel

- [ ] 3.1 `applied` in `mart_flywheel_funnel` becomes
      retrieval-conditional: a resolved trajectory counts only when its
      own run carried injected guidance, joined via `Trajectory.run_id`.
- [ ] 3.2 `applied <= retrieved <= distilled` holds by construction, and
      the column docs state what each column now counts.
      Depends on 3.1.

## 4. Version the session cursor

- [ ] 4.1 A session adapter declares a parse version, folded into its
      cursor identity, mirroring s38's `PlanAdapter::parse_version()`.
      Required rather than defaulted, so a new adapter decides
      deliberately.
- [ ] 4.2 Bumping an adapter's version makes the next
      `canon ingest sessions` re-read its transcripts instead of
      reporting `skipped unchanged (watermark)`, with no `--full` and no
      cursor deletion. Depends on 4.1.

## 5. Correct the shipped claims

- [ ] 5.1 PR #1's body: the inverted "5073 of 5079 runs are children"
      (it is 5073 roots, 6 children) and the "plan-vs-actual diff is now
      possible" claim, which was false until task 1.1.
- [ ] 5.2 Any published page repeating either claim.

## 6. Verification

- [ ] 6.1 `cargo test --workspace` green; generated-output drift clean.
- [ ] 6.2 End-to-end on this repo: dispatch a real task, dispatch a
      second run declaring the first as its parent, and `canon dispatch
      diff` classifies the edge correctly for both a declared and an
      undeclared case. Depends on 1.2 and 2.3.
- [ ] 6.3 `canon report` shows a funnel with `applied <= retrieved`.
      Depends on 3.1.
- [ ] 6.4 A session adapter version bump re-reads on the next ingest.
      Depends on 4.2.
