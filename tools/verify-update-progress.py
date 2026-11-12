"""Compile real updater callbacks in an abort-before-install, lowest-privilege fixture."""
import argparse
import ctypes
from contextlib import contextmanager
import json
import subprocess
import tempfile
import uuid
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]

@contextmanager
def retained_progress_reader(path):
    kernel=ctypes.WinDLL("kernel32",use_last_error=True)
    kernel.CreateFileW.argtypes=[ctypes.c_wchar_p,ctypes.c_uint32,ctypes.c_uint32,ctypes.c_void_p,ctypes.c_uint32,ctypes.c_uint32,ctypes.c_void_p]
    kernel.CreateFileW.restype=ctypes.c_void_p
    kernel.CloseHandle.argtypes=[ctypes.c_void_p]
    kernel.CloseHandle.restype=ctypes.c_int
    # Match Rust's reader: GENERIC_READ and FILE_SHARE_READ|WRITE|DELETE.
    handle=kernel.CreateFileW(str(path),0x80000000,7,None,3,0x80,None)
    if handle in (None,ctypes.c_void_p(-1).value):
        raise OSError(ctypes.get_last_error(),"Cannot retain the progress reader")
    try:
        yield
    finally:
        kernel.CloseHandle(handle)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    output=args.output.resolve()
    output.mkdir(parents=True,exist_ok=False)
    session=str(uuid.uuid4())
    with tempfile.TemporaryDirectory(prefix=f'companion-update-session-{session}-unicode space žluťoučký-') as temporary:
        directory=Path(temporary)
        (directory/'session-token.txt').write_text(session,encoding='ascii')
        progress=directory/'installer-progress.jsonl'
        progress.write_text('',encoding='ascii')
        fixture=output/'progress-fixture.iss'
        fixture.write_text(f'''[Setup]
AppName=Companion progress callback fixture
AppVersion=1
DefaultDirName={{tmp}}\\CompanionProgressFixture
CreateAppDir=no
Uninstallable=no
PrivilegesRequired=lowest
OutputDir={output}
OutputBaseFilename=progress-fixture
Compression=none
[Code]
#include "{(ROOT/'packaging/update-progress.iss').as_posix()}"
function InitializeSetup(): Boolean;
var Text: AnsiString; Error: String;
begin
  Error := '';
  try
    InitializeUpdateProgress();
    if not ValidUpdateSession('{session}') or ValidUpdateSession('invalid') then RaiseException('Session validation failed');
    CurInstallProgressChanged(5, 10);
    LoadStringFromFile(CompanionProgressPath, Text);
    SaveStringToFile(ExpandConstant('{{param:RESULT|}}') + '-files.jsonl', Text, False);
    CurInstallProgressChanged(10, 10);
    LoadStringFromFile(CompanionProgressPath, Text);
    SaveStringToFile(ExpandConstant('{{param:RESULT|}}') + '-extracted.jsonl', Text, False);
    WriteUpdateProgress('driver', 0, 1);
    LoadStringFromFile(CompanionProgressPath, Text);
    SaveStringToFile(ExpandConstant('{{param:RESULT|}}') + '-driver.jsonl', Text, False);
    WriteUpdateProgress('finishing', 0, 1);
    LoadStringFromFile(CompanionProgressPath, Text);
    SaveStringToFile(ExpandConstant('{{param:RESULT|}}') + '-finishing.jsonl', Text, False);
  except
    Error := GetExceptionMessage();
  end;
  SaveStringToFile(ExpandConstant('{{param:RESULT|}}') + '-error.txt', Error, False);
  Result := False;
end;
''',encoding='utf-8-sig')
        compiled=subprocess.run([str(args.compiler.resolve()),'/Q',str(fixture)],capture_output=True,text=True)
        (output/'compile.log').write_text(compiled.stdout+compiled.stderr,encoding='utf-8')
        if compiled.returncode:
            raise RuntimeError('Callback fixture did not compile; inspect compile.log')
        def run(name,token,path):
            prefix=output/name
            command=[str(output/'progress-fixture.exe'),'/VERYSILENT','/SP-','/SUPPRESSMSGBOXES',f'/COMPANIONUPDATESESSION={token}',f'/COMPANIONUPDATEPROGRESS={path}',f'/RESULT={prefix}']
            subprocess.run(command,timeout=30,check=False,capture_output=True)
            return prefix,(output/f'{name}-error.txt').read_text(encoding='utf-8-sig')
        with retained_progress_reader(progress):
            prefix,error=run('valid',session,progress)
            assert error=='',error
        snapshots={phase:Path(f'{prefix}-{phase}.jsonl').read_text(encoding='utf-8-sig').splitlines() for phase in ['files','extracted','driver','finishing']}
        assert [len(lines) for lines in snapshots.values()]==[1,2,3,4],snapshots
        records={phase:json.loads(lines[-1]) for phase,lines in snapshots.items()}
        assert records['files']==dict(schema=1,session=session,phase='installing',completed=5,total=10)
        assert records['extracted']['completed']==records['extracted']['total']==10
        assert records['driver']['phase']=='driver'
        assert records['finishing']['phase']=='finishing'
        for name,token,path in [('bad-token','invalid',progress),('wrong-session',str(uuid.uuid4()),progress),('wrong-name',session,directory/'arbitrary.txt')]:
            _,error=run(name,token,path)
            assert error,name
        (output/'result.json').write_text(json.dumps(dict(success=True,game_input=False,installed=False,elevated=False,shared_reader=True,complete_record_framing=True,records=records,rejected=['bad-token','wrong-session','wrong-name']),indent=2),encoding='utf-8')
    print(output/'result.json')

if __name__=='__main__':
    main()
