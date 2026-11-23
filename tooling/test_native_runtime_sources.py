"""Source/receipt/library admission uses synthetic bytes; actual dylib checks stay native."""
import copy
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

import native_runtime_macos as runtime


class SourceManifestTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='pttstella-runtime-source-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.exe = self.root / 'native'
        self.exe.write_bytes(b'synthetic executable; never launched')
        self.library = self.root / 'libmpv.2.dylib'
        self.library.write_bytes(b'synthetic runtime; never loaded')
        self.archive = self.root / 'source.tar.gz'
        with tarfile.open(self.archive, 'w:gz') as tar:
            for name in ['owned/LICENSE', 'owned/docs/FTL.TXT', 'owned/docs/GPLv2.TXT']:
                content = b'Own synthetic notice fixture'
                member = tarfile.TarInfo(name)
                member.size = len(content)
                tar.addfile(member, io.BytesIO(content))
        self.receipt = self.root / 'build.json'
        self.receipt.write_text(json.dumps({
            'status': 'completed', 'exit': 0, 'source_archive_sha256': runtime.digest(self.archive),
            'runtime_outputs': {self.library.name: runtime.digest(self.library)}}))
        self.manifest = self.root / 'runtime-sources.json'
        self.package = {'package': 'owned-1', 'kind': 'runtime',
                        'source': {'name': 'owned', 'versionInfo': '1',
                                   'downloadLocation': 'https://example.invalid/owned-1.tar.gz',
                                   'checksums': [{'algorithm': 'SHA256',
                                                  'checksumValue': runtime.digest(self.archive)}]},
                        'archive': self.archive.name,
                        'build_record': {'file': self.receipt.name, 'sha256': runtime.digest(self.receipt)},
                        'libraries': [{'file': self.library.name, 'sha256': runtime.digest(self.library)}]}
        self.records = {p: {'sha256': runtime.digest(p)} for p in (self.exe, self.library)}

    def write(self, packages=None):
        self.manifest.write_text(json.dumps({'schema': 1, 'packages': packages or [self.package]}))

    def collect(self):
        self.write()
        return runtime.copy_notices(self.records, self.root / 'notices', executable=self.exe,
                                    source_manifest=self.manifest)

    def test_source_receipt_and_runtime_are_bound_and_actual_notices_collected(self):
        inventory = self.collect()
        self.assertEqual(len(inventory), 1)
        self.assertEqual(inventory[0]['libraries'], [str(self.library)])
        self.assertEqual({file['file'] for file in inventory[0]['files']},
                         {'owned__LICENSE', 'owned__docs__FTL.TXT', 'owned__docs__GPLv2.TXT'})
        self.assertEqual(runtime.digest(self.root / 'notices/source-built-owned-1/build-record.json'),
                         runtime.digest(self.receipt))
        self.assertNotIn('source_delivery', inventory[0])

    def test_changed_source_receipt_or_library_is_rejected_before_output(self):
        for path in (self.archive, self.receipt, self.library):
            with self.subTest(path=path):
                original = path.read_bytes()
                path.write_bytes(original + b'changed')
                with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                    self.collect()
                self.assertFalse((self.root / 'notices').exists())
                path.write_bytes(original)

    def test_failed_build_cannot_be_admitted_by_updating_its_hash(self):
        self.receipt.write_text(json.dumps({'status': 'failed', 'exit': 1}))
        self.package['build_record']['sha256'] = runtime.digest(self.receipt)
        with self.assertRaisesRegex(ValueError, 'did not complete'):
            self.collect()
        self.assertFalse((self.root / 'notices').exists())

    def test_missing_runtime_owner_and_duplicate_ownership_are_rejected(self):
        second = copy.deepcopy(self.package)
        second['package'] = 'other-1'
        self.write([self.package, second])
        with self.assertRaisesRegex(ValueError, 'Duplicate runtime source ownership'):
            runtime.copy_notices(self.records, self.root / 'duplicate', executable=self.exe,
                                 source_manifest=self.manifest)
        self.package['libraries'] = []
        self.package['kind'] = 'build-input'
        with self.assertRaisesRegex(ValueError, 'explicit source/notice provenance'):
            self.collect()
        self.assertFalse((self.root / 'notices').exists())

    def test_unused_or_wrong_inspected_library_is_rejected(self):
        extra = self.root / 'extra.dylib'
        extra.write_bytes(b'synthetic unused file')
        self.package['libraries'] = [{'file': extra.name, 'sha256': runtime.digest(extra)}]
        with self.assertRaisesRegex(ValueError, 'not an external reachable runtime'):
            self.collect()
        self.package['libraries'] = [{'file': self.library.name, 'sha256': runtime.digest(self.library)}]
        self.records[self.library]['sha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'does not match inspected'):
            self.collect()

    def test_parent_absolute_and_symlink_escape_do_not_read_arbitrary_notices(self):
        outside = self.root.parent / (self.root.name + '-outside')
        outside.write_bytes(b'owned outside fixture')
        self.addCleanup(outside.unlink)
        link = self.root / 'escape.tar.gz'
        link.symlink_to(outside)
        for path in ('../' + outside.name, str(outside), link.name):
            with self.subTest(path=path):
                self.package['archive'] = path
                with self.assertRaises(ValueError):
                    self.collect()
                self.assertFalse((self.root / 'notices').exists())

    def test_archive_size_limit_is_checked_before_hashing(self):
        self.archive.write_bytes(b'')
        with self.archive.open('r+b') as file:
            file.truncate(runtime.SOURCE_ARCHIVE_LIMIT + 1)
        with self.assertRaisesRegex(ValueError, 'size limit'):
            self.collect()

    def test_successful_receipt_for_other_source_or_output_is_rejected(self):
        original = self.receipt.read_text()
        for key, value in [('source_archive_sha256', '0' * 64),
                           ('runtime_outputs', {self.library.name: '0' * 64})]:
            with self.subTest(key=key):
                record = json.loads(original)
                record[key] = value
                self.receipt.write_text(json.dumps(record))
                self.package['build_record']['sha256'] = runtime.digest(self.receipt)
                with self.assertRaisesRegex(ValueError, 'build record does not match'):
                    self.collect()
                self.assertFalse((self.root / 'notices').exists())

    def test_source_only_build_inputs_keep_their_notices_without_claiming_runtime_files(self):
        source_only = copy.deepcopy(self.package)
        source_only.update(package='static-input-1', kind='build-input', libraries=[])
        self.write([self.package, source_only])
        inventory = runtime.copy_notices(self.records, self.root / 'notices', executable=self.exe,
                                         source_manifest=self.manifest)
        self.assertEqual(len(inventory), 2)
        self.assertEqual(inventory[1]['kind'], 'build-input')
        self.assertEqual(inventory[1]['libraries'], [])


if __name__ == '__main__':
    unittest.main()
