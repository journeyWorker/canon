#!/usr/bin/env python3
"""Measure canon's cost against corpus size (0.29 D1).

    run.py --canon BIN --grid small|full --out results.tsv [--runs N]

Builds nothing: it runs the given `canon` binary. For each grid point it writes
a synthetic corpus (gen_corpus.py) under $TMPDIR and measures

  * `gate check` and `status` at 1x, 3x and 10x scenarios (axis `scenarios`),
  * batch `evidence add --all-scenarios --report junit:...` of B records into a
    ledger that already holds 10*B scenarios' evidence (axis `batch`, size = B).
    The ledger grows with the batch on purpose: when each record rescans the
    ledger, 10x the batch into 10x the ledger is 100x the work, and when the
    batch lands in one pass it is about 10x. Against a fixed ledger both read
    as 10x. Every run gets a fresh copy of the corpus, since landing records
    changes it.

Each measurement runs the command RUNS times, each in a fresh child so
RUSAGE_CHILDREN's peak RSS belongs to that run alone, and keeps the run with
the lowest CPU (user+sys). Output is a TSV, one row per point:

    command  axis  size  cpu_s  peak_rss_mb  wall_s  load

`load` is the 1-minute load average when the run ended; CPU and RSS, not wall,
are what check_shape.py judges. Python 3, standard library only.
"""

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent

# A feature per 25 scenarios, like canon's own corpus (24 subjects, about 600 scenarios).
PER_SUBJECT = 25
EVIDENCE_PER_SCENARIO = 3
REVIEWS_PER_SCENARIO = 2
# A batch of B records lands into a ledger of this many scenarios per record of the batch.
LEDGER_SCENARIOS_PER_BATCH_RECORD = 10

GRIDS = {
    # Each axis has a point 10x its smallest, which is what check_shape.py compares.
    "small": {"scenarios": [100, 300, 1000], "batch": [10, 30, 100]},
    "full": {"scenarios": [300, 1000, 3000], "batch": [10, 30, 100, 300]},
}

BATCH_ARGS = ["--project", "root", "--all-scenarios", "--class", "test-run", "--ref", "bench", "--role", "implementer", "--actor", "bench-batch"]

# Runs in a fresh interpreter: resource.getrusage(RUSAGE_CHILDREN) then covers exactly one command.
CHILD = (
    "import json,resource,subprocess,sys,time;"
    "t=time.time();"
    "r=subprocess.run(sys.argv[2:],cwd=sys.argv[1],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,text=True);"
    "w=time.time()-t;u=resource.getrusage(resource.RUSAGE_CHILDREN);"
    "print(json.dumps({'rc':r.returncode,'wall':w,'cpu':u.ru_utime+u.ru_stime,'rss':u.ru_maxrss/1024,'err':r.stderr[-400:]}))"
)


def load_average():
    with open("/proc/loadavg") as f:
        return f.read().split()[0]


def measure_once(argv, cwd):
    done = subprocess.run([sys.executable, "-c", CHILD, str(cwd), *argv], capture_output=True, text=True)
    if done.returncode != 0:
        sys.exit(f"measuring {argv} failed: {done.stderr.strip()}")
    result = json.loads(done.stdout)
    result["load"] = load_average()
    return result


def best_of(runs, argv, cwd, fresh=None):
    """The lowest-CPU of `runs` runs. `fresh()` returns a new directory to run in each time, when given."""
    best = None
    for _ in range(runs):
        where = fresh() if fresh else cwd
        try:
            result = measure_once(argv, where)
        finally:
            if fresh:
                shutil.rmtree(where, ignore_errors=True)
        # `gate check` exits 1 on violations, which the corpus is meant to have; 2 and up is canon failing.
        if result["rc"] > 1:
            sys.exit(f"{' '.join(argv)} exited {result['rc']}: {result['err']}")
        if best is None or result["cpu"] < best["cpu"]:
            best = result
    return best


def generate(corpus, scenarios, batch=None):
    subjects = max(2, scenarios // PER_SUBJECT)
    command = [
        sys.executable,
        str(HERE / "gen_corpus.py"),
        str(corpus),
        "--subjects", str(subjects),
        "--scenarios-per-subject", str(scenarios // subjects),
        "--evidence-per-scenario", str(EVIDENCE_PER_SCENARIO),
        "--reviews-per-scenario", str(REVIEWS_PER_SCENARIO),
    ]
    if batch:
        command += ["--staged-batch", str(batch)]
    done = subprocess.run(command, capture_output=True, text=True)
    if done.returncode != 0:
        sys.exit(f"gen_corpus.py failed: {done.stderr.strip()}")


def row(command, axis, size, result):
    return [command, axis, str(size), f"{result['cpu']:.2f}", f"{result['rss']:.0f}", f"{result['wall']:.2f}", result["load"]]


def report(rows, line):
    rows.append(line)
    print("\t".join(line), file=sys.stderr, flush=True)


def measure_scenarios(canon, work, total, runs, rows):
    corpus = work / f"scenarios-{total}"
    generate(corpus, total)
    for command in (["gate", "check"], ["status"]):
        # A first, uncounted run: the first reader of a corpus may write derived files.
        measure_once([canon, *command], corpus)
        report(rows, row(" ".join(command), "scenarios", total, best_of(runs, [canon, *command], corpus)))
    shutil.rmtree(corpus)


def measure_batch(canon, work, batch, runs, rows):
    ledger = work / f"batch-ledger-{batch}"
    generate(ledger, LEDGER_SCENARIOS_PER_BATCH_RECORD * batch, batch)
    copies = 0

    def fresh():
        nonlocal copies
        copies += 1
        target = work / f"batch-run-{batch}-{copies}"
        shutil.copytree(ledger, target, symlinks=True)
        return target

    argv = [canon, "evidence", "add", *BATCH_ARGS, "--report", f"junit:bench/batch-{batch}.xml"]
    report(rows, row("evidence add --all-scenarios", "batch", batch, best_of(runs, argv, ledger, fresh=fresh)))
    shutil.rmtree(ledger)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--canon", required=True, help="the canon binary to measure")
    parser.add_argument("--grid", choices=sorted(GRIDS), required=True)
    parser.add_argument("--out", required=True, help="results TSV")
    parser.add_argument("--only", choices=["scenarios", "batch"], help="measure one axis only")
    parser.add_argument("--runs", type=int, default=3, help="runs per point; the lowest-CPU run is kept (default 3)")
    args = parser.parse_args()
    canon = str(Path(args.canon).resolve())
    grid = GRIDS[args.grid]
    rows = [["command", "axis", "size", "cpu_s", "peak_rss_mb", "wall_s", "load"]]

    work = Path(tempfile.mkdtemp(prefix="canon-bench-"))
    try:
        if args.only in (None, "scenarios"):
            for total in grid["scenarios"]:
                measure_scenarios(canon, work, total, args.runs, rows)
        if args.only in (None, "batch"):
            for batch in grid["batch"]:
                measure_batch(canon, work, batch, args.runs, rows)
    finally:
        shutil.rmtree(work, ignore_errors=True)

    Path(args.out).write_text("".join("\t".join(r) + "\n" for r in rows))


if __name__ == "__main__":
    main()
