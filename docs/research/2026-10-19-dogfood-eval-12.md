# Dogfood eval 12: canon 0.24 (feat/0.24) on omp, roles, one name per concept (2026-10-19)

## Setup
- Repo `nyan-omp-025`: copy of `nyan-omp-024` after eval 11 (0.23 layout), skill from feat/0.24.
- Binary: feat/0.24 build `/Volumes/External/canon/bin-024/canon`, first on PATH.
- Run: `omp -p --auto-approve --max-time=4h`, prompt `@prompt-12.txt`, later turns `-c`.

## Turns
1. `prompt-12.txt` (collection screen; orchestrated roles).
2. "승인. 질문 모두 네 가정대로 진행하고, shipped까지 끝낸 다음 보고해."

## Pass criteria (plan D5c)
- 0 wrong-shape canon calls caused by naming (`evals/dogfood/trace_check.py` naming count); deprecated spellings used: count, expected 0.
- No screenshot bound to a scenario other than its own: every new evidence record's attachments are the report(s) plus files that scenario's cases name, or files bound with `--artifact <scenario-id>=<path>`.
- Shipped, `canon gate check` clean.
