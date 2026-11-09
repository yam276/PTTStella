import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

import bundle_slint_macos as bundle


class SlintCandidateBundleTests(unittest.TestCase):
    def test_existing_candidate_and_evidence_are_preserved_before_tools_run(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sentinel = root / 'retained.txt'
            sentinel.write_bytes(b'previous delivery')
            with mock.patch.object(bundle.sys, 'platform', 'darwin'), \
                    mock.patch.object(bundle, 'stage_runtime') as stage:
                with self.assertRaises(FileExistsError):
                    bundle.bundle(Path('unused'), Path('unused'), root, [])
                stage.assert_not_called()
            self.assertEqual(sentinel.read_bytes(), b'previous delivery')

    def test_wrong_main_app_profile_is_rejected_even_when_process_succeeds(self):
        wrong = {'application': 'PTTStella', 'credential_scope': 'Product',
                 'probe_credential_scope': 'SlintSyntheticProbe', 'stores_read': False,
                 'vault_opened': False}
        process = subprocess.CompletedProcess([], 0, stdout=json.dumps(wrong), stderr='')
        with mock.patch.object(bundle.subprocess, 'run', return_value=process):
            with self.assertRaisesRegex(ValueError, 'isolated Slint main-app'):
                bundle.inspect_profile(Path('synthetic-native'))

    def test_loader_and_probe_overrides_are_not_used_for_profile_check(self):
        profile = {'application': 'PTTStellaSlintResearch', 'credential_scope': 'SlintResearch',
                   'probe_credential_scope': 'SlintSyntheticProbe', 'stores_read': False,
                   'vault_opened': False}
        process = subprocess.CompletedProcess([], 0, stdout=json.dumps(profile), stderr='')
        with mock.patch.dict(bundle.os.environ, {'DYLD_LIBRARY_PATH': '/host/lib',
                                                'LD_PRELOAD': '/host/override',
                                                'PTTSTELLA_SLINT_CAPTURE_PROBE': 'native-list'}), \
                mock.patch.object(bundle.subprocess, 'run', return_value=process) as run:
            self.assertEqual(bundle.inspect_profile(Path('synthetic-native')), profile)
            environment = run.call_args.kwargs['env']
            self.assertFalse(any(key.startswith(('DYLD_', 'LD_', 'PTTSTELLA_')) for key in environment))

    def test_binary_without_native_video_cannot_be_packaged_as_full_candidate(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            license = root / 'LICENSE_SKIA'
            license.write_text('synthetic license')
            manifest = {'files': [{'file': 'MacOS/native'}]}
            with mock.patch.object(bundle.runtime, 'run', return_value='arm64\n'), \
                    mock.patch.object(bundle.runtime, 'install', return_value=manifest), \
                    mock.patch.object(bundle.bundle_common, 'copy_licenses') as notices:
                with self.assertRaisesRegex(ValueError, 'requires a bundled libmpv'):
                    bundle.stage_runtime(root / 'native', root / 'Contents', license, [])
                notices.assert_not_called()

    def test_failed_icon_conversion_leaves_no_publishable_app_or_download(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable = root / 'native'
            executable.write_bytes(b'synthetic source')

            def staged(_exe, contents, *_args):
                binary = contents / 'MacOS/native'
                binary.parent.mkdir(parents=True)
                binary.write_bytes(b'synthetic; never launched')
                (contents / 'Resources').mkdir()
                return binary, 'arm64', {'files': [{'file': 'MacOS/native',
                                                    'original_sha256': bundle.runtime.digest(executable)}]}

            output = root / 'new'
            with mock.patch.object(bundle.sys, 'platform', 'darwin'), \
                    mock.patch.object(bundle, 'stage_runtime', side_effect=staged), \
                    mock.patch.object(bundle.bundle_macos, 'make_icon', side_effect=RuntimeError('icon failed')), \
                    mock.patch.object(bundle.runtime, 'run') as run:
                with self.assertRaisesRegex(RuntimeError, 'icon failed'):
                    bundle.bundle(root / 'native', root / 'LICENSE_SKIA', output, [])
                run.assert_not_called()
            self.assertEqual(list(output.iterdir()), [])

    def test_changed_build_input_cannot_publish_a_mixed_binary_package(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable = root / 'native'
            executable.write_bytes(b'first build')

            def changed(exe, contents, *_args):
                exe.write_bytes(b'replaced build')
                return contents / 'MacOS/native', 'arm64', {'files': []}

            output = root / 'new'
            with mock.patch.object(bundle.sys, 'platform', 'darwin'), \
                    mock.patch.object(bundle, 'stage_runtime', side_effect=changed), \
                    mock.patch.object(bundle.bundle_macos, 'make_icon') as icon:
                with self.assertRaisesRegex(ValueError, 'Source executable changed'):
                    bundle.bundle(executable, root / 'LICENSE_SKIA', output, [])
                icon.assert_not_called()
            self.assertEqual(list(output.iterdir()), [])

    def test_wrong_runtime_architecture_is_rejected_before_notices_or_publication(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            license = root / 'LICENSE_SKIA'
            license.write_text('synthetic license')
            manifest = {'files': [{'file': 'MacOS/native'}, {'file': 'Frameworks/libmpv.2.dylib'}]}

            def tools(*args):
                if args[:2] == ('lipo', '-archs'):
                    return 'x86_64\n'
                raise subprocess.CalledProcessError(1, args)

            with mock.patch.object(bundle.runtime, 'run', side_effect=tools), \
                    mock.patch.object(bundle.runtime, 'install', return_value=manifest), \
                    mock.patch.object(bundle.bundle_common, 'copy_licenses') as notices:
                with self.assertRaises(subprocess.CalledProcessError):
                    bundle.stage_runtime(root / 'native', root / 'Contents', license, [])
                notices.assert_not_called()


if __name__ == '__main__':
    unittest.main()
