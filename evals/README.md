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
  --agent-cmd 'codex --full-auto "Implement the task described by CANON_EVAL_TASK_FILE"' \
  --timeout 900 \
  --result /tmp/openspec.result.json
```

The repository-default corpus is exactly 40 tasks and is pinned by
`evals/corpus.sha256`; another corpus is rejected unless
`--allow-unpinned-corpus` is explicitly supplied. The runner creates the
evaluator-owned `.canon/eval/task.json` contract and read-only
`.canon/eval/seed/` files inside the isolated workspace. Agents receive the
absolute `CANON_EVAL_TASK_FILE` and `CANON_EVAL_SEED_DIR` paths. The grader
never trusts a modified contract or seed.

Grading is dispatched through the pinned evaluator probes in `evals/probes/`,
not through commands or files supplied by the task workspace. The single
evaluation deadline covers the agent, grader, contract commands, and gate;
stdout/stderr are streamed and capped and process groups are killed on
deadline or overflow.

## Provider-neutral adapter response evidence

Adapter executions can be represented by Canon's protocol-v1 response envelope
and checked without running the provider:

```sh
canon adapter validate --response response.json --json
```

The envelope joins a `run_id` to a `context_pack_id`, records one of
`claude`, `codex`, `omp`, `pi`, or a lowercase extension slug, and carries
strict capability, evidence-digest, and telemetry fields. Provider-specific
payloads are opaque under `extensions`; validation output exposes only their
key names. OMP and Pi fixtures are supported even when those executables are
not installed locally. A file-only validation reports
`context_join_verified: null`; `--repo` can verify the referenced context pack.
This contract records declared capabilities but does not enforce them. The
provider sandbox MUST enforce filesystem, network, and secret restrictions
before execution.

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
* each required evidence JSON exists and is bound to the task, corpus, immutable
  base/current commits, and observed grader-command digest; and
* the runner-generated independent final gate has `status: "pass"`.

A zero exit code from an agent is recorded as a claim, not an outcome. If it is
followed by a failed objective grader, the result records `false_pass: true`.
Tampered result files are rejected by integrity verification. The repository
default corpus is exactly 40 unique tasks and is pinned by `evals/corpus.sha256`;
custom corpora require explicit `--allow-unpinned-corpus`. Grading uses the
SHA-pinned probes under `evals/probes/`, never a grader executable from the
task workspace.

The single evaluation deadline is shared by the agent, grader, contract
commands, and gate. Stdout/stderr are streamed and capped while each process
runs; overflow or deadline expiration kills the complete process group.

## Zero-results baseline

Before any agent run, the honest baseline is **zero results**: success@1,
false-pass rate, regression, latency, and cost are unknown (`null`), not zero and
not an improvement claim. The repository does not claim measured agent uplift
until baseline and candidate tasks have both been exercised and their result
provenance is available. A scoreboard with no result files is therefore not a
performance report; it is simply an unmeasured state.
