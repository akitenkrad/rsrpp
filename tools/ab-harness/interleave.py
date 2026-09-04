#!/usr/bin/env python3
"""Count pages a top-to-bottom sort would shuffle.

    bbox.sh <dir>          # writes <dir>/bbox/<paper>.xhtml with pdftotext
    interleave.py <dir>/bbox

A page holding two bodies of text either side of the divide, whose vertical
extents overlap, reads down its columns. Sorting such a page by `y` interleaves
them — L1, R1, L2, R2 — and the section each block belongs to goes with it.
This counts the pages where that would happen, under the document-wide rule
(what rsrpp did before 6e53cb5) and under the per-page rule (what it does now).
"""
import glob, os, re, sys
from xml.etree import ElementTree as ET

NS = "{http://www.w3.org/1999/xhtml}"
TWO_COLUMN_WIDTH_SHARE = 0.55
PAGE_LAYOUT_MIN_LINES = 10
COLUMN_BAND_MIN_CHARS = 200

def parse(path):
    raw = open(path, "rb").read()
    raw = re.sub(rb"&(?!(amp|lt|gt|quot|apos|#\d+|#x[0-9a-fA-F]+);)", b"&amp;", raw)
    raw = bytes(b for b in raw if b >= 0x20 or b in (9, 10, 13))
    return ET.fromstring(raw)

def ninth_decile(values):
    if not values:
        return None
    values = sorted(values)
    return values[-(-len(values) * 9 // 10) - 1]

def read_pages(root):
    """[(page_width, [(xmin, ymin, chars)], [line widths])]"""
    out, width = [], None
    for page in root.iter(NS + "page"):
        if width is None:
            width = float(page.get("width"))
        blocks, widths = [], []
        for block in page.iter(NS + "block"):
            lines = block.findall(NS + "line")
            if not lines:
                continue
            chars = sum(
                len("".join(w.text or "" for w in line.findall(NS + "word")))
                for line in lines
            )
            blocks.append((float(block.get("xMin")), float(block.get("yMin")), chars))
            widths += [
                float(line.get("xMax")) - float(line.get("xMin")) for line in lines
            ]
        out.append((blocks, widths))
    return width, out

def has_two_bands(blocks, half):
    left = [b[1] for b in blocks if b[0] <= half and b[2] >= COLUMN_BAND_MIN_CHARS]
    right = [b[1] for b in blocks if b[0] > half and b[2] >= COLUMN_BAND_MIN_CHARS]
    if len(left) < 2 or len(right) < 2:
        return False
    return min(left) < max(right) and min(right) < max(left)

def interleaves(blocks, half):
    left = [b for b in blocks if b[0] <= half and b[2] >= COLUMN_BAND_MIN_CHARS]
    right = [b for b in blocks if b[0] > half and b[2] >= COLUMN_BAND_MIN_CHARS]
    if len(left) < 2 or len(right) < 2:
        return False
    if not has_two_bands(blocks, half):
        return False
    ordered = sorted(left + right, key=lambda b: b[1])
    side = [0 if b[0] <= half else 1 for b in ordered]
    return sum(1 for a, b in zip(side, side[1:]) if a != b) >= 3

def main(bbox_dir):
    before = after = pages_seen = 0
    papers = set()
    for path in sorted(glob.glob(f"{bbox_dir}/*.xhtml")):
        try:
            width, pages = read_pages(parse(path))
        except ET.ParseError:
            continue
        if not width or not pages:
            continue
        half = width / 2.2
        every = [w for _, widths in pages for w in widths]
        if not every:
            continue
        document_two = ninth_decile(every) < width * TWO_COLUMN_WIDTH_SHARE
        for blocks, widths in pages:
            pages_seen += 1
            if not document_two and interleaves(blocks, half):
                before += 1
            if len(widths) >= PAGE_LAYOUT_MIN_LINES:
                page_two = ninth_decile(widths) < width * TWO_COLUMN_WIDTH_SHARE
            else:
                page_two = document_two
            page_two = page_two or has_two_bands(blocks, half)
            if not page_two and interleaves(blocks, half):
                after += 1
                papers.add(os.path.basename(path)[:-6])
    print(f"pages: {pages_seen}")
    print(f"interleaved by the document-wide rule: {before}")
    print(f"interleaved by the per-page rule:      {after}")
    if papers:
        print(f"papers still affected: {sorted(papers)}")

if __name__ == "__main__":
    main(sys.argv[1])
