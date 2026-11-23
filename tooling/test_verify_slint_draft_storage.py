from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from verify_slint_draft_storage import Runner, kill_marked_child


class OwnershipTests(unittest.TestCase):
    def test_foreign_marker_cannot_kill_either_running_child(self):
        children = [subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(20)'])
                    for _ in range(2)]
        try:
            with self.assertRaises(ValueError):
                kill_marked_child(children[0], {'pid': children[1].pid})
            self.assertTrue(all(child.poll() is None for child in children))
        finally:
            for child in children:
                if child.poll() is None:
                    child.kill()
                child.wait(timeout=10)

    def test_existing_output_preserves_its_contents(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            sentinel = output / 'keep.txt'
            sentinel.write_text('retained', encoding='utf-8')
            with self.assertRaises(FileExistsError):
                Runner(Path(sys.executable), output)
            self.assertEqual(sentinel.read_text(encoding='utf-8'), 'retained')
            self.assertEqual(sorted(p.name for p in output.iterdir()), ['keep.txt'])


if __name__ == '__main__':
    unittest.main()
