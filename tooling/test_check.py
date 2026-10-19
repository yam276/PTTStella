"""Gate regressions, not PTT terminal or renderer tests. No target files or network IO."""

import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import check


class RepositoryGateTests(unittest.TestCase):
    def test_product_suite_must_be_nonempty_without_skips_or_filters(self):
        self.assertEqual(check.require_suite('test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;'), 7)
        for output in ('', 'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;',
                       'test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;',
                       'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out;'):
            with self.subTest(output=output), self.assertRaises(check.CheckFailure):
                check.require_suite(output)

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='pttstella-gate-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / 'unrelated-checkout-name'
        for name in check.REQUIRED:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('{}' if path.suffix == '.json' else '# Fixture\n', encoding='utf-8')
        fixture = self.root / 'tests/fixtures/synthetic.json'
        fixture.parent.mkdir(parents=True)
        fixture.write_text('{"source":"synthetic"}', encoding='utf-8')
        (self.root / 'README.md').write_text('[Rules](AGENTS.md)\n', encoding='utf-8')

    def test_valid_minimal_repository(self):
        result = check.check_repository(self.root)
        self.assertEqual(result['local_file_links'], 1)
        self.assertEqual(result['fixture_json_files'], 1)

    def test_missing_required_file_fails(self):
        (self.root / 'AGENTS.md').unlink()
        with self.assertRaisesRegex(check.CheckFailure, 'required repository file'):
            check.check_repository(self.root)

    def test_cli_document_failure_returns_failed_exit(self):
        script = self.root / 'tooling/check.py'
        shutil.copyfile(check.ROOT / 'tooling/check.py', script)
        (self.root / 'README.md').write_text('[bad](missing.md)', encoding='utf-8')
        for profile in ('docs', 'all'):
            with self.subTest(profile=profile):
                result = subprocess.run([sys.executable, str(script), profile], cwd=self.root.parent,
                                        capture_output=True, text=True, encoding='utf-8', timeout=10)
                self.assertEqual(result.returncode, 1)
                report = json.loads(result.stdout)
                self.assertEqual(report['status'], 'failed')
                self.assertIn('invalid local link', report['checks'][0]['reason'])

    def test_broken_link_and_reference_definition_fail(self):
        for text in ('[bad](missing.md)', '[bad]: missing.md'):
            with self.subTest(text=text):
                (self.root / 'README.md').write_text(text, encoding='utf-8')
                with self.assertRaisesRegex(check.CheckFailure, 'invalid local link'):
                    check.check_repository(self.root)

    def test_existing_sibling_is_not_a_repository_dependency(self):
        (self.root.parent / 'sibling.md').write_text('# External\n', encoding='utf-8')
        (self.root / 'README.md').write_text('[external](../sibling.md)', encoding='utf-8')
        with self.assertRaisesRegex(check.CheckFailure, 'invalid local link'):
            check.check_repository(self.root)

    def test_malformed_link_is_a_named_failure(self):
        (self.root / 'README.md').write_text('[bad](https://[broken)', encoding='utf-8')
        with self.assertRaisesRegex(check.CheckFailure, 'malformed link'):
            check.check_repository(self.root)

    def test_fenced_examples_comments_and_remote_links_are_not_opened(self):
        text = ('````text\n```\n[example](missing.md)\n````\n'
                '<!-- [comment](also-missing.md) -->\n[remote](https://example.invalid/no-network)\n')
        (self.root / 'README.md').write_text(text, encoding='utf-8')
        self.assertEqual(check.check_repository(self.root)['local_file_links'], 0)

    def test_json_syntax_duplicate_keys_and_nonfinite_values_fail(self):
        path = self.root / 'tests/fixtures/synthetic.json'
        for text in ('{', '{"id":1,"id":2}', '{"number":NaN}', '{"number":Infinity}'):
            with self.subTest(text=text):
                path.write_text(text, encoding='utf-8')
                with self.assertRaisesRegex(check.CheckFailure, 'invalid product JSON'):
                    check.check_repository(self.root)

    def test_invalid_utf8_and_replacement_text_fail(self):
        path = self.root / 'README.md'
        for content in (b'\xff', '\ufffd'.encode('utf-8')):
            with self.subTest(content=content):
                path.write_bytes(content)
                with self.assertRaises(check.CheckFailure):
                    check.check_repository(self.root)

    def test_private_verification_is_not_inspected(self):
        local = self.root / '.local'
        local.mkdir()
        (local / 'private.json').write_text('{', encoding='utf-8')
        check.check_repository(self.root)

    def test_research_directories_are_rejected_even_if_ignored(self):
        for name in ('evidence', 'research', 'tools', '.local/evidence'):
            with self.subTest(path=name):
                directory = self.root / name
                directory.mkdir(parents=True)
                try:
                    with self.assertRaisesRegex(check.CheckFailure, 'research must be outside'):
                        check.check_repository(self.root)
                finally:
                    directory.rmdir()

    def test_research_reports_are_rejected(self):
        path = self.root / 'docs/legacy-rendering-notes.md'
        path.write_text('# Research', encoding='utf-8')
        with self.assertRaisesRegex(check.CheckFailure, 'research must be outside'):
            check.check_repository(self.root)

    def test_source_artifact_cannot_hide_under_a_product_fixture(self):
        for relative in ('tests/fixtures/legacy-client.exe', '.local/legacy-client.exe'):
            with self.subTest(path=relative):
                path = self.root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b'synthetic-marker-for-guard-test')
                try:
                    with self.assertRaisesRegex(check.CheckFailure, 'original/research artifact'):
                        check.check_repository(self.root)
                finally:
                    path.unlink()

    def test_renamed_rea_record_is_rejected_as_product_json(self):
        record = {'evidence_id': 'synthetic-id', 'subject': {}, 'provider': {},
                  'operation': 'synthetic-operation', 'raw_result': {}}
        for name, value in (('tests/fixtures/synthetic.json', record), ('renamed.json', {'records': [record]})):
            with self.subTest(name=name):
                path = self.root / name
                original = path.read_bytes() if path.exists() else None
                path.write_text(json.dumps(value), encoding='utf-8')
                try:
                    with self.assertRaisesRegex(check.CheckFailure, 'REA research record'):
                        check.check_repository(self.root)
                finally:
                    if original is None:
                        path.unlink()
                    else:
                        path.write_bytes(original)

    def test_relocated_checker_owns_its_root_from_an_unrelated_cwd(self):
        script = self.root / 'tooling/check.py'
        shutil.copyfile(check.ROOT / 'tooling/check.py', script)
        other = self.root.parent / 'other-working-directory'
        other.mkdir()
        code = ('import json,runpy,sys; m=runpy.run_path(sys.argv[1]); '
                'print(json.dumps({"root":str(m["ROOT"]),"result":m["check_repository"](m["ROOT"])}))')
        result = subprocess.run([sys.executable, '-c', code, str(script)], cwd=other,
                                capture_output=True, text=True, encoding='utf-8', timeout=10, check=True)
        self.assertEqual(Path(json.loads(result.stdout)['root']), self.root.resolve())

    def test_reserved_product_profiles_cannot_report_passed(self):
        script = self.root / 'tooling/check.py'
        shutil.copyfile(check.ROOT / 'tooling/check.py', script)
        for profile in ('core', 'desktop'):
            with self.subTest(profile=profile):
                result = subprocess.run([sys.executable, str(script), profile],
                                        capture_output=True, text=True, encoding='utf-8', timeout=10)
                self.assertEqual(result.returncode, 2)
                report = json.loads(result.stdout)
                self.assertEqual(report['status'], 'not_ready')
                self.assertEqual(report['checks'][0]['status'], 'not_ready')

    def test_required_missing_checks_cannot_be_hidden_by_document_success(self):
        results = [{'profile': 'docs', 'status': 'passed'}, check.unavailable('core'), check.unavailable('desktop')]
        self.assertEqual(check.exit_code(results), 2)
        self.assertEqual(check.exit_code(results + [{'status': 'failed'}]), 1)

    def test_packaged_executable_requires_separate_artifacts_and_desktop_scope(self):
        script = check.ROOT / 'tooling/check.py'
        for args in (['desktop', '--desktop-executable', 'missing.exe'],
                     ['docs', '--desktop-executable', 'missing.exe', '--desktop-output', 'unused']):
            with self.subTest(args=args):
                result = subprocess.run([sys.executable, str(script), *args],
                                        capture_output=True, text=True, encoding='utf-8', timeout=10)
                self.assertEqual(result.returncode, 2)
                self.assertNotIn('"status": "passed"', result.stdout)

    def test_missing_packaged_executable_cannot_fall_back_to_debug_binary(self):
        with patch.object(check.sys, 'platform', 'win32'), patch.object(check.shutil, 'which', return_value='cargo'):
            result = check.run_desktop(check.ROOT, self.root/'missing.exe', self.root/'isolated-output')
        self.assertEqual(result['status'], 'failed')
        self.assertIn('executable is missing', result['reason'])
        self.assertFalse((self.root/'isolated-output').exists())

    def test_success_exit_without_real_regression_coverage_fails(self):
        reports = ('', '{}', '[]', '{"status":"passed","tests_run":0,"failures":0,"errors":0,"skipped":0}',
                   '{"status":"passed","tests_run":1,"failures":0,"errors":0,"skipped":1}')
        for report in reports:
            with self.subTest(report=report):
                child = subprocess.CompletedProcess([], 0, stdout=report, stderr='')
                with patch.object(check.subprocess, 'run', return_value=child):
                    self.assertEqual(check.run_docs(self.root)['status'], 'failed')


if __name__ == '__main__':
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(RepositoryGateTests)
    result = unittest.TextTestRunner(verbosity=1).run(suite)
    passed = result.wasSuccessful() and result.testsRun > 0 and not result.skipped
    print(json.dumps({'status': 'passed' if passed else 'failed', 'tests_run': result.testsRun,
                      'failures': len(result.failures), 'errors': len(result.errors),
                      'skipped': len(result.skipped)}))
    raise SystemExit(0 if passed else 1)
