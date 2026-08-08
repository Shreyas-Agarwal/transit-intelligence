"""Python API: `run(mode=...)`. Mode selects a snapshot_iterator (`snapshots.py`)
and nothing else — `transform_snapshot` (`pipeline.py`) runs identically no
matter which mode supplied its input.
"""

from __future__ import annotations

import logging

from .pipeline import TransformResult, transform_snapshot
from .snapshots import SNAPSHOT_ITERATORS

logger = logging.getLogger(__name__)


def run(mode: str = "latest") -> list[TransformResult]:
    """Runs the transform pipeline over whichever snapshots `mode` supplies.

    Never raises on a per-snapshot validation failure — that's a fact about
    one snapshot, not a reason to abort the rest of the run (matters most for
    `replay`, where one bad historical snapshot shouldn't block reconstructing
    everything after it). Callers inspect each result's `.validation.passed`
    (or the CLI's exit code) to see what failed.
    """
    try:
        snapshot_iterator = SNAPSHOT_ITERATORS[mode]
    except KeyError:
        raise ValueError(
            f"unknown transform mode {mode!r}; choose one of {sorted(SNAPSHOT_ITERATORS)}"
        ) from None

    results = []
    for snapshot in snapshot_iterator():
        logger.info("transforming snapshot %s", snapshot.version)
        result = transform_snapshot(snapshot)

        if result.validation.passed:
            logger.info(
                "snapshot %s passed validation, wrote %d artifact(s) to %s",
                snapshot.version,
                len(result.artifact_row_counts or {}),
                result.silver_path,
            )
        else:
            logger.error(
                "snapshot %s failed validation:\n%s",
                snapshot.version,
                "\n".join(f"  - {f}" for f in result.validation.failures),
            )

        results.append(result)

    return results
