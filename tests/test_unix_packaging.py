import importlib.util
import plistlib
import os
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class UnixPackagingTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("unix_package", ROOT / "tools/package-rust-unix.py")
        self.package = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.package)
        (ROOT / "build/unix-package-tests").mkdir(parents=True, exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(dir=ROOT / "build/unix-package-tests")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.binary = self.root / "metinbot"
        self.binary.write_bytes(b"test executable")

    def test_linux_launcher_identity_icons_and_relocatable_command(self):
        output = self.root / "Linux package"
        self.package.stage_linux(self.binary, output)
        desktop = (output / "share/applications/app.jojo.MetinBot.desktop").read_text()
        self.assertIn("Exec=metinbot\n", desktop)
        self.assertIn("Icon=app.jojo.MetinBot\n", desktop)
        self.assertIn("StartupWMClass=app.jojo.MetinBot\n", desktop)
        for size in (16, 32, 48, 64, 128, 256, 512):
            icon = output / f"share/icons/hicolor/{size}x{size}/apps/app.jojo.MetinBot.png"
            self.assertEqual(icon.read_bytes(), (ROOT / f"packaging/icons/metinbot-{size}.png").read_bytes())
        self.assertEqual((output / "lib/metinbot/metinbot").read_bytes(), self.binary.read_bytes())
        self.assertIn('"$@"', (output / "bin/metinbot").read_text())
        self.assertIn('"$prefix/share"', (output / "install.sh").read_text())

    def test_macos_bundle_metadata_and_icon(self):
        output = self.root / "MetinBot.app"
        self.package.stage_macos(self.binary, output)
        info = plistlib.loads((output / "Contents/Info.plist").read_bytes())
        self.assertEqual(info["CFBundleIdentifier"], "app.jojo.MetinBot")
        self.assertEqual(info["CFBundleExecutable"], "metinbot")
        self.assertEqual(info["CFBundleIconFile"], "metinbot.icns")
        self.assertEqual(info["CFBundlePackageType"], "APPL")
        self.assertEqual(info["CFBundleShortVersionString"], (ROOT / "VERSION").read_text().strip())
        self.assertEqual((output / "Contents/Resources/metinbot.icns").read_bytes(), (ROOT / "packaging/icons/metinbot.icns").read_bytes())

    def test_macos_bundle_resolves_transitive_libraries_and_relocates_them(self):
        output = self.root / "MetinBot.app"
        self.package.stage_macos(self.binary, output)
        native = self.root / "native"
        native.mkdir()
        core = native / "libopencv_core.4.dylib"
        dependency = native / "libdependency.dylib"
        core.write_bytes(b"core")
        dependency.write_bytes(b"dependency")
        listing = {
            self.binary.resolve(): f"binary:\n\t@rpath/{core.name} (compatibility version 4.0.0)\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0)",
            core.resolve(): f"core:\n\t@rpath/{core.name} (compatibility version 4.0.0)\n\t{dependency.resolve()} (compatibility version 1.0.0)",
            dependency.resolve(): f"dependency:\n\t{dependency.resolve()} (compatibility version 1.0.0)",
        }
        with patch.object(self.package.subprocess, "check_output", side_effect=lambda args, **kwargs: listing[Path(args[-1])]), patch.object(self.package.subprocess, "run") as run:
            self.package.bundle_macos_libraries(self.binary, output, native)
        frameworks = output / "Contents/Frameworks"
        self.assertEqual((frameworks / core.name).read_bytes(), b"core")
        self.assertEqual((frameworks / dependency.name).read_bytes(), b"dependency")
        commands = [call.args[0] for call in run.call_args_list]
        self.assertIn(["install_name_tool", "-change", f"@rpath/{core.name}", f"@executable_path/../Frameworks/{core.name}", str(output / "Contents/MacOS/metinbot")], commands)
        self.assertIn(["install_name_tool", "-change", str(dependency.resolve()), f"@loader_path/{dependency.name}", str(frameworks / core.name)], commands)
        self.assertEqual(commands[-1], ["codesign", "--force", "--sign", "-", str(output)])

    def test_macos_unresolved_library_fails_instead_of_shipping_broken_bundle(self):
        output = self.root / "MetinBot.app"
        self.package.stage_macos(self.binary, output)
        with patch.object(self.package.subprocess, "check_output", return_value="binary:\n\t@rpath/missing.dylib (compatibility version 1.0.0)"):
            with self.assertRaises(FileNotFoundError):
                self.package.bundle_macos_libraries(self.binary, output, self.root)

    @unittest.skipUnless(os.name == "posix", "Requires a native POSIX shell")
    def test_linux_install_and_launch_preserve_spaces_and_arguments(self):
        self.binary.write_text('#!/bin/sh\nprintf "%s\\n" "$@"\nprintf "%s\\n" "$LD_LIBRARY_PATH"\n')
        output = self.root / "Linux package"
        prefix = self.root / "Installed app with spaces"
        self.package.stage_linux(self.binary, output)
        subprocess.run(["sh", str(output / "install.sh"), str(prefix)], check=True)
        env = os.environ.copy()
        env.pop("LD_LIBRARY_PATH", None)
        result = subprocess.check_output([str(prefix / "bin/metinbot"), "argument with spaces", "--flag"], env=env, text=True)
        self.assertEqual(result.splitlines(), ["argument with spaces", "--flag", str(prefix / "lib/metinbot")])


if __name__ == "__main__":
    unittest.main()
