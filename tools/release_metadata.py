"""Validated, deterministic release identity for main builds and stable tags."""
import argparse
from pathlib import Path
import re
from tools.build_metadata import BuildMetadata


def release_metadata(base:str,ref:str,run_number:int,sha:str)->dict:
    if not re.fullmatch(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)',base):
        raise ValueError('VERSION must be a stable X.Y.Z version.')
    if not 1<=run_number<=65535:raise ValueError('Run number exceeds the Windows build component range (1..65535).')
    tagged=ref.startswith('refs/tags/')
    if tagged:
        tag=ref.removeprefix('refs/tags/')
        if not tag.startswith('v'):raise ValueError('Release tag must start with v.')
        version=tag[1:]
        parsed=BuildMetadata.parse(version,run_number,sha)
        if '.'.join(map(str,parsed.windows_version[:3]))!=base:raise ValueError('Release tag must match VERSION core.')
    else:
        channel='dev' if ref=='refs/heads/main' else 'test'
        version=f'{base}-{channel}.{run_number}.{sha[:7]}'
        tag='v'+version
    build=BuildMetadata.parse(version,run_number,sha)
    stable=tagged and not build.prerelease
    return {'version':build.version,'tag':tag,'prerelease':str(not stable).lower(),
            'artifact':build.artifact_name,'commit':build.commit_sha}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ('ref','sha','output'):parser.add_argument('--'+name,required=True)
    parser.add_argument('--run-number',required=True,type=int)
    args=parser.parse_args()
    base=(Path(__file__).resolve().parent.parent/'VERSION').read_text().strip()
    values=release_metadata(base,args.ref,args.run_number,args.sha)
    with Path(args.output).open('a',encoding='utf-8') as stream:
        for name,value in values.items():stream.write(f'{name}={value}\n')


if __name__=='__main__':main()
