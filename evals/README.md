# Canon P0 maintenance evals

This corpus contains 40 distinct, real Canon maintenance tasks. Each task has an
external setup seed, instructions, expected contract, bounded diff paths,
forbidden side effects, an explicitly selected deterministic grader command,
required evidence, and a final-gate evidence contract. The corpus is intentionally
agent-neutral: Claude Code, Codex, OpenHands, or another executor can be selected
by the caller.
```sh
python3 evals/run_eval.py \
  --task-id plan-adapter-openspec-03 \
  --agent-cmd 'codex --full-auto "Implement task plan-adapter-openspec-03; inspect the CANON_EVAL_SEED_DIR environment variable and read task.json there"' \
  --timeout 900 \
  --result /tmp/openspec.result.json
```

The runner creates a detached git worktree, gives the agent a temporary seed
directory through `CANON_EVAL_SEED_DIR`, runs the task's real grader command, and
collects the diff, test output, evidence checks, and final gate. `--agent-cmd` is
required for execution mode; it is parsed as an argv command rather than run
through a shell. The command is intentionally explicit so a scoreboard can record
which executor was used. Add `--cost-usd`, `--input-tokens`, or `--output-tokens`
only when an external measurement exists; otherwise the corresponding result
fields remain `null`.

To grade a completed workspace without running an agent:

```sh
python3 evals/run_eval.py \
  --task-id stale-evidence-refusal-05 \
  --workspace /path/to/completed/git/workspace \
  --base-commit "$(git -C /path/to/completed/git/workspace rev-parse HEAD~1)" \
  --result /tmp/stale.result.json
```

A supplied semantic judgment is accepted only as explicit, provenance-bearing
JSON; the runner never invents semantic quality from agent prose:

```json
{
  "task_id": "task-id",
  "decision": "pass",
  "evidence_refs": ["review://..."],
  "provenance": {"judge": "human-or-external-evaluator", "version": "..."}
}
```

## Paired baseline/candidate scoreboard

After independently exercising both variants, compare their result files:

```sh
python3 evals/run_eval.py \
  --baseline /tmp/base-1.json /tmp/base-2.json \
  --candidate /tmp/candidate-1.json /tmp/candidate-2.json \
  --scoreboard /tmp/scoreboard.json
```

The scoreboard verifies each result's canonical SHA-256 integrity, includes result
digest provenance, reports per-task regression, success@1, false-pass count, and
latency/cost only when those values were actually supplied. Missing measurements
remain `null`; no quality or improvement is inferred from agent success text.

## Deterministic outcome contract

A task passes only when all of the following are observed:

* the task grader command exits with its declared code and does not time out;
* every changed path matches `allowed_diff_paths` (with the declared evidence
  sidecar `.canon/evidence/**` implicitly allowed) and none matches forbidden
  paths;
* every expected contract check passes;
* each required evidence JSON exists and has its declared keys; and
* final-gate evidence has `status: "pass"` and the required keys.

A zero exit code from an agent is recorded as a claim, not an outcome. If it is
followed by a failed objective grader, the result records `false_pass: true`.
Tampered result files are rejected by integrity verification. Resource control is
currently a wall-clock timeout plus bounded captured output; no hidden retries are
performed.

## Zero-results baseline

Before any agent run, the honest baseline is **zero results**: success@1,
false-pass rate, regression, latency, and cost are unknown (`null`), not zero and
not an improvement claim. The repository does not claim measured agent uplift
until baseline and candidate tasks have both been exercised and their result
provenance is available. A scoreboard with no result files is therefore not a
performance report; it is simply an unmeasured state.
