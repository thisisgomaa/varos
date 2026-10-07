"""Missing evidence must not be confused with a verified content mismatch."""
from contextlib import redirect_stdout
import io
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_vendor_patches as vendor


class VendorStatuses(unittest.TestCase):
    def test_missing_archive_skips(self):
        with tempfile.TemporaryDirectory() as cargo, patch.dict(os.environ, {"CARGO_HOME": cargo}), patch.object(sys, "argv", ["checker"]), redirect_stdout(io.StringIO()) as output:
            self.assertEqual(vendor.main(), 2)
        self.assertEqual(output.getvalue().count("SKIP (archive not cached; not verified)"), 3)
        self.assertNotIn("FAIL", output.getvalue())

    def test_archive_hash_mismatch_fails_even_when_other_archive_missing(self):
        with tempfile.TemporaryDirectory() as cargo:
            cache = Path(cargo) / "registry/cache/test"
            cache.mkdir(parents=True)
            (cache / "cosmic-text-0.19.0.crate").write_bytes(b"tampered")
            with patch.dict(os.environ, {"CARGO_HOME": cargo}), patch.object(sys, "argv", ["checker"]), redirect_stdout(io.StringIO()) as output:
                self.assertEqual(vendor.main(), 1)
        self.assertIn("archive hash mismatch", output.getvalue())
        self.assertIn("egui_tiles: SKIP", output.getvalue())
        self.assertIn("check_vendor_patches: FAIL", output.getvalue())

    def test_verified_package_passes(self):
        with patch.object(vendor, "check"), patch.object(sys, "argv", ["checker", "--package", "cosmic-text"]), redirect_stdout(io.StringIO()):
            self.assertEqual(vendor.main(), 0)


if __name__ == "__main__":
    unittest.main()
