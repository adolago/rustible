"""Bounded regression tests for the release archive size gate."""

from pathlib import Path
import tempfile
import unittest

from validate_package import validate_package


class PackagePolicyTests(unittest.TestCase):
    def test_size_boundaries(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "test.crate"
            for size in (0, 1, 10, 11):
                path.write_bytes(b"x" * size)
                with self.subTest(size=size):
                    if 1 <= size <= 10:
                        self.assertEqual(validate_package(path, limit=10), size)
                    else:
                        with self.assertRaises(ValueError):
                            validate_package(path, limit=10)

    def test_missing_archive_is_an_error(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(FileNotFoundError):
                validate_package(Path(directory) / "missing.crate")


if __name__ == "__main__":
    unittest.main()
