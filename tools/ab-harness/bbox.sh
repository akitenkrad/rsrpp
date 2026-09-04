#!/bin/bash
# usage: bbox.sh <workdir> <list> [parallel]
#   list      a file of PDF paths, one per line (see sample.py)
#   parallel  defaults to 8
#
# Writes <workdir>/bbox/<paper>.xhtml for interleave.py to read. Existing files are left
# alone, so the extraction can be resumed after an interrupted run.
S="$1"
LIST="$2"
PAR=${3:-8}
if [ -z "$S" ] || [ -z "$LIST" ]; then
  sed -n '2,4p' "$0" >&2
  exit 2
fi
mkdir -p "$S/bbox"
one() {
  f="$1"; S="$2"
  id=$(basename "$f" .pdf)
  [ -s "$S/bbox/$id.xhtml" ] || pdftotext -nopgbrk -htmlmeta -bbox-layout -r 72 "$f" "$S/bbox/$id.xhtml" 2>/dev/null
}
export -f one
cat "$LIST" | xargs -P "$PAR" -I{} bash -c 'one "$1" "$2"' _ {} "$S"
