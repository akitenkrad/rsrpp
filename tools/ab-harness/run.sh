#!/bin/bash
# usage: run.sh <binary> <outdir> <list> [parallel] [timeout_seconds]
#   list      a file of PDF paths, one per line (see sample.py)
#   parallel  defaults to 8
#   timeout   defaults to 300s per paper
#
# A per-paper time limit is not optional. One paper in the corpus (Word-derived, 17
# pages) kept pdftohtml running for 90 minutes and 2.2GB of RSS, and the whole harness
# sat behind it. The library grew its own poppler watchdog later; when it is missing, or
# when it is the thing under test, stopping a runaway is this script's responsibility.
#
# The watchdog is a one-second poll rather than a single long sleep. A long sleep
# outlives the body it was guarding, and it keeps holding the caller's end of the pipe,
# so a harness that has already finished its last paper does not return until the full
# limit has elapsed. Polling lets the guard step down the moment the body is gone.
BIN=$1; OUT=$2
LIST=$3
PAR=${4:-8}
LIMIT=${5:-300}
if [ -z "$BIN" ] || [ -z "$OUT" ] || [ -z "$LIST" ]; then
  sed -n '2,6p' "$0" >&2
  exit 2
fi
TMPROOT="${TMPROOT:-${TMPDIR:-/tmp}/rsrpp-ab}"
mkdir -p "$TMPROOT"
export LIMIT TMPROOT
run_one() {
  pdf="$1"; bin="$2"; out="$3"
  id=$(basename "$pdf" .pdf)
  log=$(mktemp)
  # Every paper gets a working directory of its own, thrown away whole when it is done.
  # The library cleans up after itself now, but not along the path that matters here: the
  # watchdog kills with SIGKILL and no destructor runs. And the size of what is left
  # behind is not bounded by anything reasonable -- one 17-page paper in the corpus, whose
  # figure is painted with tiling patterns one and four points wide, makes `pdftohtml -c`
  # write a PNG per tile: 1,079,890 files and 7.4 GB from a single run. Left in a shared
  # TMPDIR, that single paper slows down every other worker running in parallel, and
  # eventually the analysis stops making progress at all.
  paper_tmp="$TMPROOT/$id"
  rm -rf "$paper_tmp"
  mkdir -p "$paper_tmp"
  TMPDIR="$paper_tmp" "$bin" --pdf "$pdf" --out "$out/$id.json" --no-llm > "$log" 2>&1 &
  pid=$!
  (
    for ((i = 0; i < LIMIT; i++)); do
      kill -0 "$pid" 2>/dev/null || exit 0
      sleep 1
    done
    # The record is written first. Killing the body makes the parent's `wait` return, and
    # the parent then goes and kills the watchdog; with the pkill placed ahead of the
    # record, the watchdog is gone before it gets to write, and the timeout is never
    # recorded. That is not hypothetical -- it is how timed-out papers came to be logged
    # as NA.
    echo "RSRPP-TIMEOUT" >> "$log"
    kill -9 "$pid" 2>/dev/null
    # Killing rsrpp does not stop the poppler children it started; they survive their
    # parent and keep writing. The temporary directory's path is unique to this paper, so
    # it is the handle by which the grandchildren can be found and finished off.
    pkill -9 -f "$paper_tmp" 2>/dev/null
  ) >/dev/null 2>&1 &
  watchdog=$!
  wait "$pid" 2>/dev/null
  kill "$watchdog" 2>/dev/null
  wait "$watchdog" 2>/dev/null
  if grep -q RSRPP-TIMEOUT "$log"; then
    cov="TIMEOUT after ${LIMIT}s"
  else
    cov=$(grep -o "Text coverage [0-9.]*%" "$log" | head -1)
  fi
  rm -f "$log"
  # The result is recorded before the cleanup, not after. A runaway paper's temporary
  # directory holds millions of files and several gigabytes, and the `rm -rf` alone takes
  # minutes; cleaning up first means that a run interrupted in that window leaves a paper
  # that was processed but has no result to show for it. That happened on the paper that
  # ran for 90 minutes.
  echo "$id|${cov:-NA}" >> "$out/_coverage.txt"
  # Orphaned poppler processes turn up on the normal exit path too. Removing a directory
  # that something is still writing into fails with "Directory not empty", so the writer
  # is stopped before the removal is attempted.
  pkill -9 -f "$paper_tmp" 2>/dev/null
  rm -rf "$paper_tmp"
}
export -f run_one
mkdir -p "$OUT"
rm -f "$OUT/_coverage.txt"
cat "$LIST" | xargs -P "$PAR" -I{} bash -c 'run_one "$@"' _ {} "$BIN" "$OUT"
