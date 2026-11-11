"""Compile the actual Inno restart callbacks and check Windows argv/consent behavior.

The fixture returns False in InitializeSetup: it never opens a wizard, installs,
elevates, starts Companion or sends input. Reports remain under the given output.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def parse_windows_args(text):
    shell = ctypes.WinDLL("shell32", use_last_error=True)
    shell.CommandLineToArgvW.argtypes = [ctypes.c_wchar_p, ctypes.POINTER(ctypes.c_int)]
    shell.CommandLineToArgvW.restype = ctypes.POINTER(ctypes.c_wchar_p)
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.LocalFree.argtypes = [ctypes.c_void_p]
    kernel.LocalFree.restype = ctypes.c_void_p
    count = ctypes.c_int()
    pointer = shell.CommandLineToArgvW("companion.exe " + text, ctypes.byref(count))
    if not pointer:
        raise OSError(ctypes.get_last_error())
    try:
        return [pointer[i] for i in range(count.value)]
    finally:
        kernel.LocalFree(pointer)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if os.name != "nt":
        raise SystemExit("This acceptance check requires Windows.")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    data = output / "profiles with spaces žluťoučký"
    data.mkdir()
    include = ROOT / "packaging/update-restart.iss"
    migration_include = ROOT / "packaging/installation-migration.iss"
    fixture = output / "restart-fixture.iss"
    fixture.write_text(f'''[Setup]
AppName=Companion restart callback fixture
AppVersion=1
DefaultDirName={{tmp}}\\CompanionRestartFixture
CreateAppDir=no
Uninstallable=no
PrivilegesRequired=lowest
OutputDir={output}
OutputBaseFilename=restart-fixture
Compression=none
[Code]
#include "{include.as_posix()}"
#include "{migration_include.as_posix()}"
function Hex(Value: String): String;
var I, N, Code: Integer; Part, Digits: String;
begin
  Result := '';
  Digits := '0123456789ABCDEF';
  for I := 1 to Length(Value) do
  begin
    Code := Ord(Value[I]);
    Part := '';
    for N := 1 to 4 do
    begin
      Part := Digits[(Code mod 16) + 1] + Part;
      Code := Code div 16;
    end;
    Result := Result + Part;
  end;
end;
function InitializeSetup(): Boolean;
var Text, Requested: String;
begin
  if ExpandConstant('{{param:SNAPSHOTROOT|}}') <> '' then
    if not SnapshotPreviousInventory(ExpandConstant('{{param:SNAPSHOTROOT|}}'),
      ExpandConstant('{{param:SNAPSHOTROOT|}}') + '\\files.json', ExpandConstant('{{param:SNAPSHOTOUTPUT|}}')) then
      RaiseException('Snapshot fixture failed');
  Requested := '0';
  if UpdateRestartRequested() then Requested := '1';
  Text := Hex(UpdateRestartArguments('')) + #13#10 + Requested + #13#10 +
    Hex(PreferredInstallationDirectory(ExpandConstant('{{param:PREVIOUS|}}'), ExpandConstant('{{param:DEFAULT|}}'))) + #13#10 +
    Hex(PreflightInstallationDirectory(ExpandConstant('{{param:PREVIOUS|}}'), ExpandConstant('{{param:TARGET|C:\\CompanionCallbackFixture}}'), False)) + #13#10 +
    Hex(PendingPreviousInstallation(ExpandConstant('{{param:PREVIOUS|}}'), ExpandConstant('{{param:TARGET|C:\\CompanionCallbackFixture}}'), ExpandConstant('{{param:PENDINGSOURCE|}}'), ExpandConstant('{{param:PENDINGTARGET|}}')));
  SaveStringToFile(ExpandConstant('{{param:RESULT|}}'), Text, False);
  Result := False;
end;
''', encoding="utf-8-sig")
    compiled = subprocess.run([str(args.compiler.resolve()), "/Q", str(fixture)], capture_output=True, text=True)
    (output / "compile.log").write_text(compiled.stdout + compiled.stderr, encoding="utf-8")
    if compiled.returncode:
        raise RuntimeError("Restart fixture did not compile; inspect compile.log")
    directory = str(data)
    extended = "\\\\?\\" + directory
    cases = [
        ("spaces_unicode", directory, True, False, True),
        ("trailing_slash", directory + "\\", True, False, True),
        ("extended_path", extended, True, False, True),
        ("drive_root", data.anchor, True, False, True),
        ("missing_consent", directory, False, False, False),
        ("silent", directory, True, True, False),
        ("missing_directory", str(output / "absent"), True, False, False),
    ]
    results = []
    for name, root, consent, silent, expected in cases:
        result = output / (name + ".txt")
        command = [str(output / "restart-fixture.exe"), "/SP-", "/SUPPRESSMSGBOXES", f"/RESULT={result}", f"/COMPANIONDATADIR={root}"]
        if consent:
            command.append("/RESTARTCOMPANION=1")
        if silent:
            command.append("/VERYSILENT")
        subprocess.run(command, timeout=30, check=False, capture_output=True)
        lines = result.read_text(encoding="ascii").splitlines()
        parameters = bytes.fromhex(lines[0]).decode("utf-16-be")
        parsed = parse_windows_args(parameters)
        assert parsed[:2] == ["companion.exe", "--data-dir"] and len(parsed) == 3, name
        # Inno's incoming parameter parser may retain doubled trailing separators.
        # The contract is the same Windows directory, including drive-root syntax.
        assert os.path.normpath(parsed[2]) == os.path.normpath(root), name
        if expected:
            assert os.path.samefile(parsed[2], root), name
        assert (lines[1] == "1") == expected, name
        results.append({"case": name, "argv_roundtrip": True, "restart_requested": expected})
    directory_cases = [
        ("fresh", "", r"C:\Program Files\Companion", r"C:\Program Files\Companion"),
        ("old_default", "C:\\Program Files\\MetinBot\\", r"C:\Program Files\Companion", r"C:\Program Files\Companion"),
        ("existing_new", r"D:\Apps\Companion", r"C:\Program Files\Companion", r"D:\Apps\Companion"),
        ("custom_old_parent", r"D:\My Apps\MetinBot", r"C:\Program Files\Companion", r"D:\My Apps\Companion"),
        ("custom_unrelated", r"D:\Custom", r"C:\Program Files\Companion", r"D:\Custom"),
    ]
    for name, previous, default, expected in directory_cases:
        result = output / ("directory_" + name + ".txt")
        subprocess.run([str(output / "restart-fixture.exe"), "/SP-", "/SUPPRESSMSGBOXES", f"/RESULT={result}", f"/PREVIOUS={previous}", f"/DEFAULT={default}"], timeout=30, check=False, capture_output=True)
        lines = result.read_text(encoding="ascii").splitlines()
        chosen = bytes.fromhex(lines[2]).decode("utf-16-be")
        assert os.path.normpath(chosen) == os.path.normpath(expected), (name, chosen)
    preflight = output / "preflight"
    preflight.mkdir()
    previous = preflight / "MetinBot"
    previous.mkdir()
    (previous / "files.json").write_text("[]")
    target = preflight / "Companion"
    target.mkdir()
    (target / "private.txt").write_text("keep unchanged")
    preflight_cases = [
        ("fresh", "", str(preflight / "fresh"), True),
        ("occupied", str(previous), str(target), False),
        ("overlap", str(previous), str(previous / "Companion"), False),
        ("old_inplace", str(previous), str(previous), True),
        ("retry", str(target), str(target), True),
    ]
    for name, old, new, expected in preflight_cases:
        result = output / ("preflight_" + name + ".txt")
        subprocess.run([str(output / "restart-fixture.exe"), "/SP-", "/SUPPRESSMSGBOXES", f"/RESULT={result}", f"/PREVIOUS={old}", f"/TARGET={new}"], timeout=30, check=False, capture_output=True)
        lines = result.read_text(encoding="ascii").splitlines()
        error = bytes.fromhex(lines[3]).decode("utf-16-be") if len(lines) > 3 else ""
        assert (not error) == expected, (name, error)
        if name == "retry":
            pending = bytes.fromhex(lines[4]).decode("utf-16-be")
            assert os.path.normpath(pending) == os.path.normpath(str(target)), "An unrelated sibling must not become a pending source"
    assert (target / "private.txt").read_text() == "keep unchanged"
    # Only an explicit recorded AppId source permits a pending relocation retry.
    for name, source, pending_target, expected in [
        ("recorded", str(previous), str(target), str(previous)),
        ("other_target", str(previous), str(preflight / "other"), str(target)),
        ("unrelated_sibling", "", "", str(target)),
    ]:
        result = output / ("pending_" + name + ".txt")
        subprocess.run([str(output / "restart-fixture.exe"), "/SP-", "/SUPPRESSMSGBOXES", f"/RESULT={result}", f"/PREVIOUS={target}", f"/TARGET={target}", f"/PENDINGSOURCE={source}", f"/PENDINGTARGET={pending_target}"], timeout=30, check=False, capture_output=True)
        lines = result.read_text(encoding="ascii").splitlines()
        chosen = bytes.fromhex(lines[4]).decode("utf-16-be")
        assert os.path.normpath(chosen) == os.path.normpath(expected), (name, chosen)
    # Execute the exact pre-write snapshot routine without installing anything.
    license_dir = previous / 'licenses' / 'old unicode \u017elu\u0165ou\u010dk\u00fd'
    license_dir.mkdir(parents=True)
    notice = license_dir / 'NOTICE.txt'
    notice.write_bytes(b'original historical notice')
    (license_dir / 'unknown.dll').write_bytes(b'unlisted binary must not become a notice')
    original_inventory = [{"path":"MetinBot.exe", "sha256":"f" * 64}]
    (previous / 'files.json').write_text(json.dumps(original_inventory), encoding='utf8')
    snapshot = output / 'prewrite-snapshot.json'
    subprocess.run([str(output / "restart-fixture.exe"), "/SP-", "/SUPPRESSMSGBOXES", f"/RESULT={output / 'snapshot.txt'}", f"/SNAPSHOTROOT={previous}", f"/SNAPSHOTOUTPUT={snapshot}"], timeout=30, check=False, capture_output=True)
    captured = json.loads(snapshot.read_text(encoding='utf8'))
    assert len(captured['archive_id']) == 64 and all(c in '0123456789abcdef' for c in captured['archive_id'])
    assert captured['inventory'] == original_inventory
    assert captured['historical_notices'] == [{"path":str(notice.relative_to(previous)), "sha256":hashlib.sha256(notice.read_bytes()).hexdigest()}]
    link = preflight / "redirected-parent"
    linked = subprocess.run(["cmd.exe", "/C", "mklink", "/J", str(link), str(preflight)], capture_output=True, text=True)
    if linked.returncode:
        raise RuntimeError("Cannot create isolated junction fixture: " + linked.stderr)
    try:
        result = output / "preflight_junction.txt"
        subprocess.run([str(output / "restart-fixture.exe"), "/SP-", "/SUPPRESSMSGBOXES", f"/RESULT={result}", f"/TARGET={link / 'new'}"], timeout=30, check=False, capture_output=True)
        lines = result.read_text(encoding="ascii").splitlines()
        assert bytes.fromhex(lines[3]).decode("utf-16-be"), "Junction parent was accepted"
    finally:
        link.rmdir()
    report = {"success": True, "cases": results, "directory_migration_cases": len(directory_cases), "preflight_cases": len(preflight_cases) + 1, "pending_journal_cases": 3, "prewrite_snapshot_verified": True, "installer_executed": False, "game_input_sent": False}
    (output / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
