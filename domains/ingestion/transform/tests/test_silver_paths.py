from __future__ import annotations

from ingestion.transform.silver_paths import SilverLayout


def test_publish_moves_staging_dir_to_final_path(silver_root):
    layout = SilverLayout(silver_root)
    staging = layout.staging_dir("20260805")
    staging.mkdir(parents=True)
    (staging / "stops.parquet").write_bytes(b"fake parquet bytes")

    final_dir = layout.publish(staging, "20260805")

    assert final_dir == silver_root / "20260805"
    assert (final_dir / "stops.parquet").exists()
    assert not staging.exists()


def test_advance_latest_points_symlink_at_version(silver_root):
    layout = SilverLayout(silver_root)
    layout.final_dir("20260805").mkdir(parents=True)

    layout.advance_latest_if_newer("20260805")

    assert layout.current_latest_version() == "20260805"


def test_advance_latest_never_moves_backwards(silver_root):
    layout = SilverLayout(silver_root)
    layout.final_dir("20260729").mkdir(parents=True)
    layout.final_dir("20260805").mkdir(parents=True)

    layout.advance_latest_if_newer("20260805")
    layout.advance_latest_if_newer("20260729")  # older — must not regress `latest`

    assert layout.current_latest_version() == "20260805"


def test_advance_latest_is_idempotent_for_the_same_version(silver_root):
    layout = SilverLayout(silver_root)
    layout.final_dir("20260805").mkdir(parents=True)

    layout.advance_latest_if_newer("20260805")
    layout.advance_latest_if_newer("20260805")

    assert layout.current_latest_version() == "20260805"
