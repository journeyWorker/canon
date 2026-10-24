#!/usr/bin/env python3
"""Write a synthetic canon repo for scaling measurements (0.29 D1).

    gen_corpus.py OUT --subjects S --scenarios-per-subject N \
        --evidence-per-scenario E --reviews-per-scenario R [--staged-batch B ...]

OUT becomes a self-contained git repo canon accepts: `canon.yaml`, a policy
that requires evidence, a failure case per feature and reviews, one feature
(`.canon/specs/kind=feature/area=<area>/<surface>.feature`) and one subject
per S, N tagged scenarios per feature, E evidence records and R review
records per scenario, all committed.

Records are written straight into `.canon/ledger` with the file names the git
tier derives (`<natural key>__<content_digest12>.json`, partition.rs), so
generating thousands of records takes seconds and the output is
byte-deterministic: fixed timestamps, fixed commit dates, no randomness. The
derived scenario index (gitignored, normally rebuilt by `canon inventory
sync`) is written too, so the first `canon` run measures steady state instead
of an index rebuild.

The corpus is shaped so the per-scenario paths run: every 10th scenario has
no evidence (uncovered cell), every 7th scenario's latest evidence is at an
older commit than a change to the surface it declares (stale evidence),
the other scenarios are fresh with R reviews pinned to what they saw, and
half the subjects are `verifying` (review gate on) and half `building`, and
every 2nd subject adopts a chore change with an approved brief (the gate and
`status` then derive the code tree for it).

`--staged-batch B` (repeatable) also commits `bench/batch-B.xml`, a JUnit
report with one case for each of B scenarios spread across the corpus. The
batch entry point under test is
`canon evidence add --project-id root --all-scenarios --report junit:bench/batch-B.xml ...`
(`land_batch` in canon-cli/src/evidence.rs): B records landing in a ledger
that already holds S*N*E.

Python 3, standard library only.
"""

import argparse
import datetime
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

PROJECT = "root"
SPEC_ROOT = ".canon/specs"
SURFACE_FILE = "src/core.txt"
EPOCH = datetime.datetime(2026, 10, 1, tzinfo=datetime.timezone.utc)
# When the human approved each chore brief: after the first evidence round, so
# scenarios last evidenced on older code have a trigger.
CHORE_APPROVED_AT = 600
GIT_DATES = ("2026-10-01T00:00:00+00:00", "2026-10-02T00:00:00+00:00", "2026-10-03T00:00:00+00:00")

CANON_YAML = """\
tiers:
  local:
    backend: git
    root: .canon/ledger
  hot:
    backend: sqlite
    path: .canon/hot.db
routing:
  change: local
  task: hot
  scenario: local
  session: hot
  run: hot
  event: hot
  handoff: hot
  review: local
  divergence: local
  trajectory: local
  strategy_item: local
  evidence_record: local
  subject: local
  finding: local
specs:
  roots:
    - id: root
      root: .canon/specs
plans:
  sources: []
write:
  promote: auto
"""

POLICY_YAML = """\
# Scaffolded like `canon init`'s policy: evidence, a failure case per feature, reviews.
spec_coverage:
  require_evidence: true
  require_cases: [failure]
  require_review: {}
"""

# What `canon init` ignores; the ledger kinds this corpus writes stay tracked.
GITIGNORE = """\
.canon/hot.db*
.canon/audit/
.canon/ledger/kind=scenario/
.canon/archive/kind=scenario/
.canon/scenario-index.lock
.canon/artifact-store.lock
.canon/dashboard-snapshot/
"""


def stamp(n):
    return (EPOCH + datetime.timedelta(seconds=n)).strftime("%Y-%m-%dT%H:%M:%SZ")


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def digest12(record):
    """canon-store `content_digest12`: sha256 over the canonical (sorted-key, compact) JSON."""
    body = json.dumps(record, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return sha256(body.encode())[:12]


def write_record(root, kind, area, natural_key, record, archive=False):
    name = f"{natural_key}__{digest12(record)}.json"
    directory = root / ".canon" / ("archive" if archive else "ledger") / f"kind={kind}"
    if area:
        directory = directory / f"area={area}"
    directory.mkdir(parents=True, exist_ok=True)
    (directory / name).write_text(json.dumps(record, sort_keys=True, indent=2), encoding="utf-8")


def git(root, *args, env=None):
    full = dict(os.environ)
    full.update(
        GIT_AUTHOR_NAME="bench",
        GIT_AUTHOR_EMAIL="bench@example.invalid",
        GIT_COMMITTER_NAME="bench",
        GIT_COMMITTER_EMAIL="bench@example.invalid",
        GIT_CONFIG_GLOBAL="/dev/null",
        GIT_CONFIG_SYSTEM="/dev/null",
    )
    full.update(env or {})
    done = subprocess.run(["git", *args], cwd=root, env=full, capture_output=True, text=True)
    if done.returncode != 0:
        sys.exit(f"git {' '.join(args)} failed: {done.stderr.strip()}")
    return done.stdout.strip()


def commit(root, message, date):
    git(root, "add", "-A")
    git(root, "commit", "-q", "--allow-empty", "-m", message, env={"GIT_AUTHOR_DATE": date, "GIT_COMMITTER_DATE": date})
    return git(root, "rev-parse", "HEAD")


def code_tree(root, sha):
    """The tree canon records as `code_tree`: the commit without `.canon/` (git_rev.rs `code_tree`)."""
    index = Path(tempfile.mkdtemp(prefix="bench-index-")) / "index"
    try:
        env = {"GIT_INDEX_FILE": str(index)}
        git(root, "read-tree", sha, env=env)
        git(root, "rm", "-r", "--cached", "-q", "--", ".canon", env=env)
        return git(root, "write-tree", env=env)
    finally:
        shutil.rmtree(index.parent, ignore_errors=True)


def subject_id(s):
    return f"sub-{s:04d}"


def scenario_id(s, n):
    return f"{area_of(s)}.{subject_id(s)}.{n + 1:02d}"


def area_of(s):
    return f"ar{s % 8}"


PROVENANCE = '  # canon: {"schema":1,"at":"2026-10-01T00:00:00Z","actor":{"agent_id":"bench-gen"}}\n'


def scenario_lines(s, n, per_subject):
    """(tags, title, steps) of one scenario; the last scenario of a feature is its failure case."""
    sid = scenario_id(s, n)
    tags = [f"@{sid}", f"@subject:{subject_id(s)}"]
    if n == per_subject - 1:
        tags.append("@case:failure")
    return sid, tags, f"Scenario: Bench scenario {s}-{n}", [f"When the actor does thing {n}", f"Then the result {n} holds"]


def feature_text(s, per_subject):
    out = [f"Feature: bench feature {s}\n", PROVENANCE]
    for n in range(per_subject):
        _, tags, title, steps = scenario_lines(s, n, per_subject)
        out.append("\n" + PROVENANCE)
        out.append("  " + " ".join(tags) + "\n")
        out.append(f"  {title}\n")
        out.extend(f"    {step}\n" for step in steps)
    return "".join(out)


def scenario_digest(s, n, per_subject):
    """canon-fmt `scenario_digests` for this template (no feature tags, Background or Rule)."""
    _, tags, title, steps = scenario_lines(s, n, per_subject)
    scenario = [" ".join(sorted(tags)), title, *steps]
    text = "canon-scenario-digest/1"
    for label, part in (("feature-tags", []), ("background", []), ("rule", []), ("rule-background", []), ("scenario", scenario)):
        text += f"\n[{label}]" + "".join(f"\n{line}" for line in part)
    return sha256(text.encode())


def evidence_body(clock, sid, run_seq, sha, tree, verdict, session):
    return {
        "actor": {"agent_id": "bench-impl", "role": "implementer", "session_id": session},
        "at": stamp(clock),
        "code_tree": tree,
        "evidence": {"kind": "test-run", "ref": "bench"},
        "evidence_sha": sha,
        "kind": "evidence_record",
        "project_id": PROJECT,
        "run_seq": run_seq,
        "scenario_id": sid,
        "schema": 1,
        "staging_id": sha256(f"{sid}/{run_seq}".encode())[:12],
        "surface_ref": [SURFACE_FILE],
        "verdict": verdict,
        "worktree_dirty": False,
    }


def review_body(clock, sid, reviewer, pin, tree, digest, verdict):
    return {
        "actor": {"agent_id": reviewer, "role": "reviewer", "session_id": f"{reviewer}-session"},
        "at": stamp(clock),
        "code_tree": tree,
        "kind": "review",
        "pin": pin,
        "pins": {
            "checks": [{"checks": [], "role": "implementer", "verdict": verdict}],
            "scenario_digest": digest,
            "scenario_digest_scope": "scenario",
        },
        "project_id": PROJECT,
        "provenance_ref": {"upstream_ref": "bench"},
        "reviewer": reviewer,
        "scenario_id": sid,
        "schema": 1,
        "worktree_dirty": False,
    }


CHORE_BRIEF = """\
# Chore for {sid}

<!-- canon:kind chore -->

<!-- canon:section intent role=context -->
## Intent

Keep the build of {sid} tidy.

<!-- canon:section category role=context -->
## Category

refactor

<!-- canon:section invariants role=normative -->
## Invariants

The behavior of every scenario of {sid} does not change.

<!-- canon:section checks role=normative -->
## Checks

The bench suite is run again at the chore commit.

<!-- canon:section out-of-scope role=context -->
## Out of scope

None.
"""


def chore_id(s):
    return f"chore-{subject_id(s)}"


def approved_chore_brief(s):
    """The brief with the `Approved:` line `canon brief approve` writes, bound to the text above it."""
    text = CHORE_BRIEF.format(sid=subject_id(s)) + "\n"
    digest = sha256(text.encode())
    return text + f'Approved: bench-human {stamp(CHORE_APPROVED_AT)} \u2014 "go" (brief sha256:{digest})\n'


def junit(scenarios):
    cases = "".join(f'  <testcase classname="bench" name="{sid} batch case"/>\n' for sid in scenarios)
    return f"<testsuite>\n{cases}</testsuite>\n"


def generate(out, subjects, per_subject, evidence_n, reviews_n, batches, uncovered_every, stale_every, chore_every):
    out = Path(out)
    if out.exists() and any(out.iterdir()):
        sys.exit(f"{out} exists and is not empty")
    out.mkdir(parents=True, exist_ok=True)
    git(out, "init", "-q", "-b", "main")

    (out / "canon.yaml").write_text(CANON_YAML)
    (out / ".gitignore").write_text(GITIGNORE)
    (out / ".canon").mkdir(exist_ok=True)
    (out / ".canon" / "policy.yaml").write_text(POLICY_YAML)
    (out / "src").mkdir()
    (out / SURFACE_FILE).write_text("core v1\n")
    (out / "src" / "other.txt").write_text("other\n")

    clock = 0
    sources = {}
    for s in range(subjects):
        area = area_of(s)
        feature = out / SPEC_ROOT / "kind=feature" / f"area={area}" / f"{subject_id(s)}.feature"
        feature.parent.mkdir(parents=True, exist_ok=True)
        text = feature_text(s, per_subject)
        feature.write_text(text)
        sources[s] = sha256(text.encode())
        clock += 1
        chored = bool(chore_every) and s % chore_every == 0
        if chored:
            brief = out / ".canon" / "changes" / chore_id(s) / "brief.md"
            brief.parent.mkdir(parents=True, exist_ok=True)
            brief.write_text(approved_chore_brief(s), encoding="utf-8")
            write_record(
                out,
                "change",
                None,
                chore_id(s),
                {
                    "actor": {"agent_id": "bench-gen"},
                    "at": stamp(clock),
                    "change_id": chore_id(s),
                    "kind": "change",
                    "schema": 2,
                    "status": "proposed",
                    "subject_ids": [subject_id(s)],
                    "summary": f"Chore for {subject_id(s)}",
                    "title": chore_id(s),
                },
            )
        write_record(
            out,
            "subject",
            None,
            subject_id(s),
            {
                "actor": {"agent_id": "bench-gen", "role": "implementer"},
                "at": stamp(clock),
                **({"change_ids": [chore_id(s)]} if chored else {}),
                "domain": "bench",
                "kind": "subject",
                "owner_role": "implementer",
                "schema": 1,
                "status": "verifying" if s % 2 == 0 else "building",
                "subject_id": subject_id(s),
                "summary": "",
                "title": f"Bench subject {s}",
            },
        )

    every = [(s, n) for s in range(subjects) for n in range(per_subject)]
    for batch in batches:
        picked = [scenario_id(*every[i * len(every) // batch]) for i in range(batch)] if batch <= len(every) else None
        if picked is None:
            sys.exit(f"--staged-batch {batch} exceeds the {len(every)} scenarios")
        (out / "bench").mkdir(exist_ok=True)
        (out / "bench" / f"batch-{batch}.xml").write_text(junit(picked))

    first = commit(out, "bench corpus: specs, subjects, code", GIT_DATES[0])
    first_tree = code_tree(out, first)
    (out / SURFACE_FILE).write_text("core v2\n")
    second = commit(out, "bench corpus: the declared surface changes", GIT_DATES[1])
    second_tree = code_tree(out, second)

    covered = []
    for index, (s, n) in enumerate(every):
        sid, _, title, _ = scenario_lines(s, n, per_subject)
        digest = scenario_digest(s, n, per_subject)
        clock += 1
        index_record = {
            "actor": {"agent_id": "canon-inventory-sync"},
            "at": stamp(clock),
            "kind": "scenario",
            "project_id": PROJECT,
            "scenario_digest": digest,
            "scenario_id": sid,
            "schema": 1,
            "source_digest": sources[s],
            "subject_id": subject_id(s),
            "title": title.removeprefix("Scenario: "),
        }
        if n == per_subject - 1:
            index_record["case"] = "failure"
        write_record(out, "scenario", area_of(s), f"{PROJECT}__{sid}", index_record)
        if evidence_n > 0 and not (uncovered_every and index % uncovered_every == 3 % uncovered_every):
            covered.append((index, s, sid, digest, bool(stale_every) and index % stale_every == 0))

    # Evidence in rounds, so `at` and `run_seq` (one counter per feature surface, as
    # promotion assigns them) grow together. A write moves what it supersedes to
    # `.canon/archive` (0.23 "latest only"), so every round but the last lands there.
    run_seq = {}
    for round_n in range(1, evidence_n + 1):
        latest = round_n == evidence_n
        for index, s, sid, _, stale in covered:
            clock += 1
            run_seq[s] = run_seq.get(s, 0) + 1
            # Older rounds are at the first commit; the latest is at the second unless this scenario is stale.
            sha, tree = (first, first_tree) if stale or not latest else (second, second_tree)
            # An older run alternates failing and passing; the latest always passes.
            verdict = "faithful" if latest or round_n % 2 == 0 else "divergent"
            record = evidence_body(clock, sid, run_seq[s], sha, tree, verdict, f"impl-{index % 7}")
            write_record(out, "evidence_record", None, sid, record, archive=not latest)
    for _, s, sid, digest, _ in covered:
        for r in range(reviews_n):
            clock += 1
            write_record(out, "review", area_of(s), f"{PROJECT}__{sid}__{second}", review_body(clock, sid, f"bench-rev-{r + 1}", second, second_tree, digest, "faithful"))

    head = commit(out, "bench corpus: evidence and reviews", GIT_DATES[2])
    summary = {
        "out": str(out),
        "subjects": subjects,
        "scenarios": len(every),
        "evidence_per_scenario": evidence_n,
        "reviews_per_scenario": reviews_n,
        "staged_batches": batches,
        "head": head,
    }
    print(json.dumps(summary))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("out", help="directory to create (must not exist or be empty)")
    parser.add_argument("--subjects", type=int, required=True)
    parser.add_argument("--scenarios-per-subject", type=int, required=True)
    parser.add_argument("--evidence-per-scenario", type=int, required=True)
    parser.add_argument("--reviews-per-scenario", type=int, required=True)
    parser.add_argument("--staged-batch", type=int, action="append", default=[], metavar="B", help="also commit bench/batch-B.xml, a JUnit report for B scenarios (repeatable)")
    parser.add_argument("--uncovered-every", type=int, default=10, help="every Nth scenario has no evidence (0: none)")
    parser.add_argument("--stale-every", type=int, default=7, help="every Nth scenario's latest evidence predates a surface change (0: none)")
    parser.add_argument("--chore-every", type=int, default=2, help="every Nth subject adopts a chore change with an approved brief (0: none)")
    args = parser.parse_args()
    for name in ("subjects", "scenarios_per_subject"):
        if getattr(args, name) < 1:
            parser.error(f"--{name.replace('_', '-')} must be at least 1")
    generate(args.out, args.subjects, args.scenarios_per_subject, args.evidence_per_scenario, args.reviews_per_scenario, sorted(set(args.staged_batch)), args.uncovered_every, args.stale_every, args.chore_every)


if __name__ == "__main__":
    main()
