#!/usr/bin/env python3
"""Fail when canon's cost grows faster than the corpus does (0.29 D1).

    check_shape.py results.tsv [--max-factor 20]

Reads run.py's TSV. For each (command, axis) it takes the smallest size as 1x
and the size ten times as large as 10x, and divides the 10x point's CPU and
peak RSS by the 1x point's. Work linear in the corpus gives about 10, and a
fixed start-up cost lowers it; work that rescans the corpus once per item gives
about 100. A factor above --max-factor (default 20) exits 1 and prints the
offending row. A missing 10x point exits 2.

CPU below FLOOR_CPU_S is read as FLOOR_CPU_S: the TSV rounds to 10 ms and a
ratio of two such readings says nothing. Python 3, standard library only.
"""

import argparse
import csv
import sys
from collections import defaultdict

FLOOR_CPU_S = 0.05
FLOOR_RSS_MB = 1.0


def read(path):
    with open(path, newline="") as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    groups = defaultdict(dict)
    for r in rows:
        groups[(r["command"], r["axis"])][int(r["size"])] = r
    return groups


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("results", help="TSV written by run.py")
    parser.add_argument("--max-factor", type=float, default=20.0, help="largest allowed 10x/1x ratio of CPU and of peak RSS (default 20)")
    args = parser.parse_args()

    groups = read(args.results)
    if not groups:
        sys.exit(f"{args.results} has no rows")
    failed = False
    missing = False
    print(f"{'command':<30}{'axis':<11}{'1x':>6}{'10x':>6}{'cpu 1x':>9}{'cpu 10x':>9}{'cpu x':>8}{'rss 1x':>8}{'rss 10x':>9}{'rss x':>8}")
    for (command, axis), points in sorted(groups.items()):
        small = min(points)
        if small * 10 not in points:
            print(f"{command} ({axis}): no point at 10x of {small}", file=sys.stderr)
            missing = True
            continue
        one, ten = points[small], points[small * 10]
        cpu_factor = max(float(ten["cpu_s"]), FLOOR_CPU_S) / max(float(one["cpu_s"]), FLOOR_CPU_S)
        rss_factor = max(float(ten["peak_rss_mb"]), FLOOR_RSS_MB) / max(float(one["peak_rss_mb"]), FLOOR_RSS_MB)
        print(f"{command:<30}{axis:<11}{small:>6}{small * 10:>6}{one['cpu_s']:>9}{ten['cpu_s']:>9}{cpu_factor:>8.1f}{one['peak_rss_mb']:>8}{ten['peak_rss_mb']:>9}{rss_factor:>8.1f}")
        for what, factor in (("cpu", cpu_factor), ("rss", rss_factor)):
            if factor > args.max_factor:
                failed = True
                one_row, ten_row = "\t".join(one.values()), "\t".join(ten.values())
                print(f"FAIL {command} ({axis}): 10x costs {factor:.1f}x the 1x {what}, over {args.max_factor:g}x\n  1x:  {one_row}\n  10x: {ten_row}", file=sys.stderr)
    if missing:
        sys.exit(2)
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
