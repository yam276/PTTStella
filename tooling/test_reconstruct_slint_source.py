"""Source delivery integrity with an independently authored, local Git fixture."""
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

import reconstruct_slint_source as source


class ReconstructionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='pttstella-source-test-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.repo = self.root / 'fixture'
        self.repo.mkdir()
        self.environment = {k: v for k, v in os.environ.items() if not k.startswith('GIT_')}
        self.environment.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull)
        self.git('init', '--quiet', '--object-format=sha1')
        for name, text in {
            'Cargo.toml': '[package]\nname="source-fixture"\nversion="0.1.0"\n',
            'Cargo.lock': 'version = 4\n', 'build.rs': 'fn main() {}\n',
            'src/lib.rs': 'pub fn answer() -> u32 { 42 }\n',
            'README.md': 'Self-authored source reconstruction fixture.\n',
        }.items():
            path = self.repo / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding='utf-8')
        (self.repo / 'assets').mkdir()
        (self.repo / 'assets/fixture.dat').write_bytes(b'\x00\x01synthetic source asset')
        self.git('add', '.')
        self.git('-c', 'user.name=yam276', '-c',
                 'user.email=59956724+yam276@users.noreply.github.com',
                 'commit', '--quiet', '-m', 'test: independent source fixture')
        self.bundle = self.root / 'product.bundle'
        self.git('bundle', 'create', str(self.bundle), 'HEAD')
        self.manifest_path = self.root / 'sources.json'
        self.output = self.root / 'restored'
        self.manifest = {
            'format': 'PTTStella.CandidateProductSource', 'version': 1,
            'bundle': {'file': self.bundle.name, 'sha256': source.digest(self.bundle)},
            'candidates': {'fixture': {
                'base_commit': self.git('rev-parse', 'HEAD'),
                'source_tree': self.git('write-tree'), 'files': self.inventory(),
            }},
        }

    def git(self, *arguments):
        return subprocess.run(['git', *arguments], cwd=self.repo,
                              env=self.environment, check=True, capture_output=True,
                              text=True).stdout.strip()

    def inventory(self):
        return {name: source.digest(self.repo / name)
                for name in self.git('ls-files', '-z').rstrip('\0').split('\0')}

    def run_source(self):
        self.manifest_path.write_text(json.dumps(self.manifest), encoding='utf-8')
        return source.reconstruct(self.manifest_path, 'fixture', self.output)

    def test_complete_tree_is_restored_without_the_original_checkout(self):
        expected = self.inventory()
        shutil.rmtree(self.repo)
        result = self.run_source()
        self.assertEqual(result['status'], 'product_source_reconstructed')
        self.assertEqual(result['files_verified'], 6)
        self.assertEqual({name: source.digest(self.output / name) for name in expected}, expected)
        self.assertFalse(self.repo.exists())
        self.assertIn('no third-party closure', result['scope'])

    def test_binary_patch_and_added_file_match_the_recorded_tree(self):
        (self.repo / 'src/lib.rs').write_bytes(b'pub const ANSWER: u32 = 43;\n')
        (self.repo / 'src/added.rs').write_bytes(b'// independent new source\n')
        (self.repo / 'assets/fixture.dat').write_bytes(b'\x00\xffchanged synthetic source asset')
        self.git('add', 'src', 'assets')
        patch = self.root / 'changes.patch'
        patch.write_bytes(subprocess.check_output(
            ['git', 'diff', '--cached', '--binary', 'HEAD'], cwd=self.repo,
            env=self.environment))
        self.assertIn(b'GIT binary patch', patch.read_bytes())
        record = self.manifest['candidates']['fixture']
        record.update(source_tree=self.git('write-tree'), files=self.inventory(),
                      patch={'file': patch.name, 'sha256': source.digest(patch)})
        result = self.run_source()
        self.assertEqual(result['files_verified'], 7)
        self.assertEqual((self.output / 'src/lib.rs').read_bytes(), b'pub const ANSWER: u32 = 43;\n')
        self.assertEqual((self.output / 'src/added.rs').read_bytes(), b'// independent new source\n')
        self.assertEqual((self.output / 'assets/fixture.dat').read_bytes(),
                         b'\x00\xffchanged synthetic source asset')

    def test_corrupt_or_symlink_bundle_is_rejected_before_creating_output(self):
        original = self.bundle.read_bytes()
        self.bundle.write_bytes(original + b'changed')
        with self.assertRaisesRegex(ValueError, 'hash mismatch'):
            self.run_source()
        self.assertFalse(self.output.exists())
        self.bundle.unlink()
        other = self.root / 'other.bundle'
        other.write_bytes(original)
        self.bundle.symlink_to(other)
        with self.assertRaisesRegex(ValueError, 'bounded regular file'):
            self.run_source()
        self.assertFalse(self.output.exists())

    def test_existing_destination_is_preserved(self):
        self.output.mkdir()
        marker = self.output / 'owned.txt'
        marker.write_bytes(b'keep this exact text')
        with self.assertRaisesRegex(ValueError, 'destination already exists'):
            self.run_source()
        self.assertEqual(marker.read_bytes(), b'keep this exact text')
        link = self.root / 'destination-link'
        link.symlink_to(self.output, target_is_directory=True)
        self.output = link
        with self.assertRaisesRegex(ValueError, 'destination already exists'):
            self.run_source()

    def test_invalid_paths_and_incomplete_inventory_are_rejected(self):
        original = copy.deepcopy(self.manifest)
        for name in ['../escape', '/absolute', '.git/config', 'src/../escape',
                     'src\\escape', 'C:escape']:
            with self.subTest(name=name):
                self.manifest = copy.deepcopy(original)
                self.manifest['candidates']['fixture']['files'][name] = '0' * 64
                with self.assertRaises(ValueError):
                    self.run_source()
                self.assertFalse(self.output.exists())
        self.manifest = copy.deepcopy(original)
        del self.manifest['candidates']['fixture']['files']['Cargo.lock']
        with self.assertRaisesRegex(ValueError, 'inventory is incomplete'):
            self.run_source()
        self.assertFalse(self.output.exists())

    def test_wrong_tree_or_file_hash_cannot_claim_reconstruction(self):
        original = copy.deepcopy(self.manifest)
        for key in ['tree', 'file', 'omitted']:
            with self.subTest(key=key):
                self.manifest = copy.deepcopy(original)
                self.output = self.root / ('bad-' + key)
                record = self.manifest['candidates']['fixture']
                if key == 'tree':
                    record['source_tree'] = '0' * 40
                elif key == 'file':
                    record['files']['src/lib.rs'] = hashlib.sha256(b'wrong').hexdigest()
                else:
                    del record['files']['README.md']
                with self.assertRaises(ValueError):
                    self.run_source()


if __name__ == '__main__':
    unittest.main()
