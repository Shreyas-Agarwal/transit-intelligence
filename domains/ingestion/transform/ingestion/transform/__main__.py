"""CLI: `python -m ingestion.transform [latest|replay]` (defaults to `latest`)."""

from __future__ import annotations

import argparse
import logging
import sys

from .run import run
from .snapshots import SNAPSHOT_ITERATORS


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m ingestion.transform")
    parser.add_argument(
        "mode",
        choices=sorted(SNAPSHOT_ITERATORS),
        nargs="?",
        default="latest",
        help="which snapshots to transform (default: latest)",
    )
    args = parser.parse_args(argv)

    logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")

    results = run(mode=args.mode)
    failed = [r for r in results if not r.validation.passed]

    print(f"Processed {len(results)} snapshot(s), {len(failed)} failed validation")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
