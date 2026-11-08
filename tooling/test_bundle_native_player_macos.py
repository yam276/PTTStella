import json
from pathlib import Path
import tempfile
import unittest

from bundle_native_player_macos import fixtures


class FixtureTests(unittest.TestCase):
    def test_manifest_cannot_request_arbitrary_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / 'source'
            source.mkdir()
            (source / 'manifest.json').write_text(json.dumps({'license': 'CC0-1.0', 'files': [
                {'file': '../private.mp4', 'bytes': 3, 'sha256': 'x'}]}))
            with self.assertRaises(ValueError):
                fixtures(source, root / 'out')
            self.assertFalse((root / 'out').exists())


if __name__ == '__main__':
    unittest.main()
