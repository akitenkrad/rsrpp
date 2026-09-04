#!/bin/bash
# Counts, over a corpus, the papers on which the section-placement pass does not fire.
#
# usage: census.sh <binary> <outfile> <list> [parallel] [timeout_seconds]
#   list      a file of PDF paths, one per line (see sample.py)
#   parallel  defaults to 8
#   timeout   defaults to 300s per paper
#
# Output, one line per paper:
#   <paper-id>|<candidates>|<stood down for the whole document>|<rejected individually>
#
# The candidate count is read out of the debug log that `detect_sections` already emits,
# "Section detection: ... N title-font entries". Counting it that way means the
# implementation does not have to be touched in order to be measured.
#
# Working directories are per-paper and thrown away when the paper is done, for the
# reason given in run.sh. The library removes its own working directory now, but the
# watchdog's kill is a SIGKILL and no destructor runs on that path, so whatever it misses
# is cleaned up here.
BIN=$1; OUT=$2
LIST=$3
PAR=${4:-8}
LIMIT=${5:-300}
if [ -z "$BIN" ] || [ -z "$OUT" ] || [ -z "$LIST" ]; then
  sed -n '3,7p' "$0" >&2
  exit 2
fi
TMPROOT="${TMPROOT:-${TMPDIR:-/tmp}/rsrpp-census}"
mkdir -p "$TMPROOT"
export LIMIT TMPROOT
census_one() {
  pdf="$1"; bin="$2"; out="$3"
  id=$(basename "$pdf" .pdf)
  log=$(mktemp)
  paper_tmp="$TMPROOT/$id"
  rm -rf "$paper_tmp"; mkdir -p "$paper_tmp"
  TMPDIR="$paper_tmp" RUST_LOG=rsrpp=debug \
    "$bin" --pdf "$pdf" --out "$paper_tmp/out.json" --no-llm > "$log" 2>&1 &
  pid=$!
  (
    for ((i = 0; i < LIMIT; i++)); do
      kill -0 "$pid" 2>/dev/null || exit 0
      sleep 1
    done
    echo "RSRPP-TIMEOUT" >> "$log"
    kill -9 "$pid" 2>/dev/null
    pkill -9 -f "$paper_tmp" 2>/dev/null
  ) >/dev/null 2>&1 &
  watchdog=$!
  wait "$pid" 2>/dev/null
  kill "$watchdog" 2>/dev/null
  wait "$watchdog" 2>/dev/null
  if grep -q RSRPP-TIMEOUT "$log"; then
    echo "$id|TIMEOUT|-|-" >> "$out"
  else
    n=$(grep -o '[0-9]\+ title-font entries' "$log" | head -1 | grep -o '^[0-9]\+')
    stood_down=$(grep -c 'Skipping the placement pass' "$log")
    rejected=$(grep -c ' — left .* matches no column margin' "$log")
    echo "$id|${n:-NA}|$stood_down|$rejected" >> "$out"
  fi
  rm -f "$log"
  pkill -9 -f "$paper_tmp" 2>/dev/null
  rm -rf "$paper_tmp"
}
export -f census_one
rm -f "$OUT"
cat "$LIST" | xargs -P "$PAR" -I{} bash -c 'census_one "$@"' _ {} "$BIN" "$OUT"
