#!/usr/bin/env python3
"""Launch-free real QA module import at host and isolated mount paths."""
from pathlib import Path
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().with_name("megamail-native-batch2-qa.py")
CODE = SOURCE.read_text()

class ImportTests(unittest.TestCase):
    def load(self, filename):
        namespace = {"__file__": filename, "__name__": "qa_import_contract"}
        with patch("subprocess.Popen", side_effect=AssertionError("unexpected launch")), patch("subprocess.run", side_effect=AssertionError("unexpected launch")):
            exec(compile(CODE, filename, "exec"), namespace)
        return namespace

    def test_host_location_preserves_repository_root(self):
        self.assertEqual(self.load(str(SOURCE))["ROOT"], SOURCE.parent.parent)

    def test_root_mount_import_does_not_abort_or_launch(self):
        namespace = self.load("/qa-run.py")
        self.assertEqual(namespace["ROOT"], Path("/"))
        args = namespace["parser"]().parse_args(["--inside", "--execute"])
        self.assertTrue(args.inside)
        self.assertTrue(args.execute)

if __name__ == "__main__":
    unittest.main()
