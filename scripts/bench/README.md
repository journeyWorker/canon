# Corpus-size bench

Does `canon gate check`, `canon status` or a batch `canon evidence add` cost
more than the corpus it reads? Three stdlib-only Python 3 scripts answer it
for any `canon` binary.

| Script | Does |
|---|---|
| `gen_corpus.py OUT --subjects S --scenarios-per-subject N --evidence-per-scenario E --reviews-per-scenario R [--staged-batch B]` | Writes a deterministic git repo canon accepts. |
| `run.py --canon BIN --grid small\|full --out results.tsv` | Generates corpora under `$TMPDIR`, measures the binary, writes a TSV. |
| `check_shape.py results.tsv [--max-factor 20]` | Exits 1 when 10x the corpus costs more than 20x the CPU or the peak RSS of 1x. |

```sh
cargo build --release -p canon-cli --bin canon
python3 scripts/bench/run.py --canon target/release/canon --grid small --out /tmp/bench.tsv
python3 scripts/bench/check_shape.py /tmp/bench.tsv
```

## The corpus

`gen_corpus.py` writes records straight into `.canon/ledger` under the names
the git tier derives (`<key>__<content_digest12>.json`), so thousands of
records take seconds, and the same arguments give the same bytes and the same
commit shas. The scenario index (gitignored, normally rebuilt by `canon
inventory sync`) is written too: `inventory sync` on a corpus reports 0 records
written, so the first run measures steady state. It holds:

- a feature file and a subject for each of S subjects (half `verifying`, half
  `building`), N scenarios each, the last tagged `@case:failure`;
- E evidence records per scenario. The ledger keeps the latest, as 0.23's
  latest-only ledger does, and the other E-1 are in `.canon/archive`. Every
  10th scenario has none (uncovered cell), every 7th has its latest at a commit
  older than a change to the file it declares (stale evidence);
- R reviews per scenario, by R reviewers, pinned to what they saw;
- a chore change with an approved brief for every 2nd subject, which makes the
  gate and `status` derive the code tree;
- with `--staged-batch B`, `bench/batch-B.xml`, a JUnit report with one case for
  each of B scenarios spread over the corpus. The batch entry point is
  `canon evidence add --project root --all-scenarios --report junit:bench/batch-B.xml --class test-run --ref bench --role implementer --actor bench-batch`
  (`land_batch` in `crates/canon-cli/src/evidence.rs`).

The policy asks for evidence, a failure case per feature and reviews, so the
gate reports violations. That is intended: the per-scenario checks have
something to report.

## The grid

`run.py` measures each point in a fresh child process, three times, and keeps
the run with the lowest CPU (user+sys); peak RSS is `RUSAGE_CHILDREN`'s for that
run. Wall time and the 1-minute load average are recorded but never judged.

| Axis | Command | Sizes (`small`) | Sizes (`full`) | What grows |
|---|---|---|---|---|
| `scenarios` | `gate check`, `status` | 100, 300, 1000 | 300, 1000, 3000 | subjects, scenarios, evidence, reviews together (25 scenarios per subject, E=3, R=2) |
| `batch` | `evidence add --all-scenarios` | 10, 30, 100 | 10, 30, 100, 300 | B records into a ledger of 10*B scenarios |

The batch ledger grows with B on purpose. Landing B records that each rescan the
ledger costs B times the ledger: 100x for 10x the batch into 10x the ledger,
about 10x when the batch lands in one pass. Against a fixed ledger both would
read as 10x, and the check could not tell them apart.

## Reading the result

Work linear in the corpus gives a factor of about 10, a fixed start-up cost
lowers it, and work repeated once per item gives about 100. The 20x limit is
between them. `check_shape.py` reads the smallest size of each command and axis
as 1x and the size ten times larger as 10x, floors CPU at 0.05 s and RSS at
1 MB, and prints one line per command with both factors. The `bench-shape` job
in `.github/workflows/test.yml` runs the `small` grid against the release binary.
