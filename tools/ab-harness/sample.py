#!/usr/bin/env python3
"""Draw the paper sample the A/B runs go over. Fixed seed, so the same papers come back.

    sample.py <corpus-dir> [--seed N] [--count N] > list.txt
    RSRPP_AB_CORPUS=<corpus-dir> sample.py [--seed N] [--count N] > list.txt

The corpus is yours to supply: a directory of PDFs to measure against. There is no
default, because a list of paths that happens to work on one machine is worse than no
list at all -- it makes an A/B run that measured nothing look like it measured something.
"""
import argparse, glob, os, random, sys

def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("corpus", nargs="?", default=os.environ.get("RSRPP_AB_CORPUS"),
                        help="directory of PDFs (or set RSRPP_AB_CORPUS)")
    parser.add_argument("--seed", type=int, default=20260903)
    parser.add_argument("--count", type=int, default=300)
    args = parser.parse_args()
    if not args.corpus:
        parser.print_usage(sys.stderr)
        print("sample.py: give a corpus directory, or set RSRPP_AB_CORPUS", file=sys.stderr)
        return 2
    pdfs = sorted(glob.glob(os.path.join(os.path.expanduser(args.corpus), "*.pdf")))
    if not pdfs:
        print(f"sample.py: no PDFs under {args.corpus}", file=sys.stderr)
        return 1
    random.seed(args.seed)
    picked = pdfs if args.count >= len(pdfs) else random.sample(pdfs, args.count)
    print("\n".join(picked))
    return 0

if __name__ == "__main__":
    sys.exit(main())
