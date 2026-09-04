# A/B harness

A change to a parser is not finished when the tests pass. The tests cover the cases
someone thought of; a corpus of real papers covers the ones nobody did. This harness runs
two builds of `rsrpp` — one from before a change, one from after — over the same set of
PDFs, and reports what moved. **The completion condition for a parsing change is not
"the tests are green" but "measured over the corpus, nothing regressed."**

Bring your own corpus. The scripts take a list of PDF paths; what is in that list is up
to you. Nothing here ships paper files or paths to them.

## Scripts

| Script | What it does |
| --- | --- |
| `sample.py <corpus-dir> [--seed N] [--count N]` | Prints a reproducible random sample of PDF paths from a directory. Same seed, same papers. Also reads the directory from `RSRPP_AB_CORPUS`. |
| `run.sh <binary> <outdir> <list> [parallel] [timeout]` | Parses every paper in the list with one build, writing `<outdir>/<paper-id>.json` and a `<outdir>/_coverage.txt`. Each paper runs under its own time limit and its own working directory. |
| `compare.py <old_dir> <new_dir>` | Reports the differences between two `run.sh` output directories: section lists that changed, coverage that dropped, papers whose text got shorter. |
| `census.sh <binary> <outfile> <list> [parallel] [timeout]` | Counts, per paper, how many section-title candidates were found and whether the placement pass stood down. Reads the numbers out of the existing debug log, so measuring costs no change to the implementation. |
| `bbox.sh <workdir> <list> [parallel]` | Extracts word-level geometry with `pdftotext -bbox-layout` into `<workdir>/bbox/`. |
| `interleave.py <workdir>/bbox` | Over that geometry, counts the pages a top-to-bottom sort would shuffle — the column-detection question, answered without running `rsrpp` at all. |

## A typical run

Build the two binaries from two commits rather than from the working tree. A worktree
keeps the "old" build honest: it is the commit the change started from, not whatever the
stash happened to hold.

```sh
cargo build --release && cp target/release/rsrpp /tmp/rsrpp-new

git worktree add /tmp/rsrpp-base <base-commit>
(cd /tmp/rsrpp-base && cargo build --release && cp target/release/rsrpp /tmp/rsrpp-old)

tools/ab-harness/sample.py ~/corpus --count 300 > /tmp/ab-list.txt
tools/ab-harness/run.sh /tmp/rsrpp-old /tmp/ab-old /tmp/ab-list.txt 8
tools/ab-harness/run.sh /tmp/rsrpp-new /tmp/ab-new /tmp/ab-list.txt 8
tools/ab-harness/compare.py /tmp/ab-old /tmp/ab-new

git worktree remove /tmp/rsrpp-base
```

Measured on a 14-core machine: 300 papers at eight in parallel takes about ten minutes a
side, and 2,800 papers at twelve takes about half an hour. Raise the parallelism to the
number of cores you are willing to give up; the per-paper timeout is what keeps one
pathological document from holding the rest hostage.

Sample a few hundred papers while iterating, and run the whole corpus before calling the
change done. Both of the regressions that mattered most in this parser's history — a
stamp along the margin becoming the first section, and a document whose layout changes
partway through — appeared only in the larger run.

## Reading the result

`compare.py` counts "papers losing characters" by comparing the concatenated body text.
That count is usually an overstatement: text that moved into the drop ledger has left the
sections but not the document. When a paper shows up as a loss, re-run those two builds
with `--keep-dropped` and compare again before calling it a regression.

## Files this produces

Lists of paper paths and the output directories are machine-local and generated; they are
git-ignored rather than committed.
