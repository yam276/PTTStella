"""Execute the production build script with controlled native/cross library discovery."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


class MpvBuildTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        rustc = shutil.which('rustc')
        if not rustc:
            raise unittest.SkipTest('rustc is required to execute the real build script')
        cls.temporary = tempfile.TemporaryDirectory(prefix='pttstella-mpv-build-')
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.root = Path(cls.temporary.name)
        suffix = '.exe' if os.name == 'nt' else ''
        cls.builder = cls.root / ('builder' + suffix)
        cls.no_video = cls.root / ('no-video' + suffix)
        cls.tools = cls.root / 'tools'
        cls.tools.mkdir()
        # A real host executable avoids shell/PATH differences on native Windows.
        fake = cls.root / 'pkg-config.rs'
        fake.write_text('''fn main() {
    std::fs::write(std::env::var_os("TEST_PKG_MARKER").unwrap(), "called").unwrap();
    if std::env::var_os("TEST_PKG_FAIL").is_some() { std::process::exit(17); }
    assert_eq!(std::env::args().skip(1).collect::<Vec<_>>(), ["--variable=libdir", "mpv"]);
    println!("{}", std::env::var("TEST_PKG_LIBDIR").unwrap());
}''', encoding='utf-8')
        for command in [
            [rustc, '--edition=2024', '--cfg', 'feature="native-video"',
             str(ROOT / 'build.rs'), '-o', str(cls.builder)],
            [rustc, '--edition=2024', str(ROOT / 'build.rs'), '-o', str(cls.no_video)],
            [rustc, '--edition=2024', '--crate-name', 'pkg_config', str(fake),
             '-o', str(cls.tools / ('pkg-config' + suffix))],
        ]:
            subprocess.run(command, capture_output=True, text=True, check=True, timeout=60)
        details = subprocess.run([rustc, '-vV'], capture_output=True, text=True,
                                 check=True, timeout=30).stdout
        cls.host = next(line.removeprefix('host: ') for line in details.splitlines()
                        if line.startswith('host: '))
        cls.foreign = ('x86_64-pc-windows-msvc' if cls.host != 'x86_64-pc-windows-msvc'
                       else 'aarch64-apple-darwin')

    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='case-', dir=self.root)
        self.addCleanup(temporary.cleanup)
        self.case = Path(temporary.name)
        self.host_lib = self.case / 'host libraries'
        self.host_lib.mkdir()
        self.target_lib = self.case / 'target libraries'
        self.target_lib.mkdir()
        self.marker = self.case / 'pkg-called'
        self.environment = dict(os.environ)
        for name in ['MPV_LIB_DIR', 'PTTSTELLA_NOTICES_FILE', 'TEST_PKG_FAIL']:
            self.environment.pop(name, None)
        self.environment.update({
            'HOST': self.host, 'TARGET': self.host,
            'OUT_DIR': str(self.case), 'TEST_PKG_MARKER': str(self.marker),
            'TEST_PKG_LIBDIR': str(self.host_lib),
            'PATH': str(self.tools) + os.pathsep + os.environ.get('PATH', ''),
        })

    def run_script(self, binary=None):
        return subprocess.run([str(binary or self.builder)], cwd=ROOT,
                              env=self.environment, capture_output=True, text=True, timeout=15)

    def test_cross_build_does_not_consume_host_pkg_config(self):
        self.environment['TARGET'] = self.foreign
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn('MPV_LIB_DIR', result.stderr)
        self.assertFalse(self.marker.exists(), 'cross build executed host pkg-config')
        self.assertNotIn('cargo:rustc-link-search=', result.stdout)

    def test_native_build_uses_pkg_config(self):
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.marker.is_file())
        self.assertIn(f'cargo:rustc-link-search=native={self.host_lib}', result.stdout)
        self.assertTrue((self.case / 'third-party-notices.txt').is_file())

    def test_cross_override_uses_explicit_target_directory(self):
        self.environment.update(TARGET=self.foreign, MPV_LIB_DIR=str(self.target_lib))
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(self.marker.exists())
        self.assertIn(f'cargo:rustc-link-search=native={self.target_lib}', result.stdout)
        self.assertNotIn(str(self.host_lib), result.stdout)

    def test_invalid_override_does_not_fall_back_to_host(self):
        self.environment['MPV_LIB_DIR'] = str(self.case / 'missing')
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('MPV library directory is unavailable', result.stderr)
        self.assertFalse(self.marker.exists())

    def test_pkg_config_failure_is_reported(self):
        self.environment['TEST_PKG_FAIL'] = '1'
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('native-video requires libmpv', result.stderr)
        self.assertTrue(self.marker.exists())

    def test_missing_cargo_target_identity_does_not_guess_native(self):
        for name in ['HOST', 'TARGET']:
            with self.subTest(name=name):
                value = self.environment.pop(name)
                result = self.run_script()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(name, result.stderr)
                self.assertFalse(self.marker.exists())
                self.environment[name] = value

    def test_no_video_build_does_not_query_native_library(self):
        self.environment['TARGET'] = self.foreign
        result = self.run_script(self.no_video)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(self.marker.exists())
        self.assertNotIn('cargo:rustc-link-search=', result.stdout)


if __name__ == '__main__':
    unittest.main()
