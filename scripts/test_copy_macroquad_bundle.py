"""Regression coverage for the Cargo bundle helper (#329)."""

import subprocess
import sys
import unittest
from unittest.mock import patch

import copy_macroquad_bundle as bundle


class CargoMetadataTests(unittest.TestCase):
    def test_utf8_metadata_and_diagnostics_do_not_depend_on_windows_locale(self):
        original_run = subprocess.run

        def cargo_output(*args, **kwargs):
            self.assertEqual(kwargs.get("encoding"), "utf-8")
            return original_run(
                [sys.executable, "-c", "import sys; sys.stdout.buffer.write(b'{\"name\":\"\\xe9\\xba\\xbb\\xe9\\x9b\\x80\"}')"],
                **kwargs,
            )

        with patch.object(bundle.subprocess, "run", side_effect=cargo_output):
            self.assertEqual(bundle.load_cargo_metadata(), {"name": "麻雀"})

        def cargo_error(*args, **kwargs):
            self.assertEqual(kwargs.get("encoding"), "utf-8")
            return original_run(
                [sys.executable, "-c", "import sys; sys.stderr.buffer.write(b'\\xe6\\x8b\\x92\\xe5\\x90\\xa6'); sys.exit(1)"],
                **kwargs,
            )

        with patch.object(bundle.subprocess, "run", side_effect=cargo_error):
            with self.assertRaisesRegex(RuntimeError, "拒否"):
                bundle.load_cargo_metadata()


if __name__ == "__main__":
    unittest.main()
