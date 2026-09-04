#!/usr/bin/env python3
"""Compare two A/B output directories.

    compare.py <old_dir> <new_dir>

Each directory holds one <paper-id>.json per paper plus a _coverage.txt of
"<paper-id>|Text coverage NN.N%" lines, as written by run.sh.

Reports what actually matters: section lists that changed, coverage that got
worse, and papers whose output lost characters. A paper that "loses text" here
has usually only moved fragments into the drop ledger — re-run those two builds
with --keep-dropped before calling it a loss.
"""
import json, glob, os, re, sys, collections

def coverage(d):
    out = {}
    for line in open(f"{d}/_coverage.txt"):
        key, value = line.strip().split("|", 1)
        found = re.search(r"([0-9.]+)%", value)
        out[key] = float(found.group(1)) if found else None
    return out

def titles(d, paper):
    return [x.get("title", "") for x in json.load(open(f"{d}/{paper}.json"))]

def text(d, paper):
    blocks = []
    for section in json.load(open(f"{d}/{paper}.json")):
        for field in ("contents", "captions"):
            value = section.get(field)
            if isinstance(value, list):
                blocks += [str(x) for x in value]
    return re.sub(r"\s+", "", "".join(blocks))

def main(old, new):
    papers = [os.path.basename(p)[:-5] for p in sorted(glob.glob(f"{new}/*.json"))]
    old_cov, new_cov = coverage(old), coverage(new)
    same = [p for p in papers if titles(old, p) == titles(new, p)]
    worse = [p for p in papers
             if old_cov.get(p) and new_cov.get(p) and new_cov[p] - old_cov[p] < -0.05]
    better = [p for p in papers
              if old_cov.get(p) and new_cov.get(p) and new_cov[p] - old_cov[p] > 0.05]
    lost = [p for p in papers if len(text(new, p)) < len(text(old, p)) - 2]
    counts = collections.Counter(len(titles(new, p)) - len(titles(old, p)) for p in papers)

    print(f"papers: {len(papers)}")
    print(f"section lists identical: {len(same)}")
    print(f"coverage worse: {len(worse)}  better: {len(better)}")
    print(f"papers losing characters: {len(lost)}")
    print(f"section-count delta: {dict(sorted(counts.items()))}")
    for p in worse[:10]:
        print(f"  WORSE {p}: {old_cov[p]} -> {new_cov[p]}")
    for p in lost[:10]:
        print(f"  LOST  {p}: {len(text(old, p))} -> {len(text(new, p))}")
    for p in papers:
        a, b = titles(old, p), titles(new, p)
        if a != b:
            gone, came = set(a) - set(b), set(b) - set(a)
            if gone or came:
                print(f"  DIFF  {p}: lost {sorted(gone)} gained {sorted(came)}")

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
