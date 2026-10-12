"""Inspect release identity, bundled dependencies and native elevation manifest."""
import argparse
import json
from pathlib import Path
import xml.etree.ElementTree as ET
import pefile


def verify_bundle(directory:Path,version:str,commit:str)->None:
    internal=directory/'_internal'
    info=json.loads((internal/'build-info.json').read_text())
    if info['version']!=version or info['commit_sha']!=commit.lower():raise ValueError('Bundled release identity mismatch.')
    for name in ('python314.dll','_tkinter.pyd','cv2/cv2.pyd','_tcl_data/init.tcl','_tk_data/tk.tcl'):
        if not (internal/name).is_file():raise ValueError(f'Missing bundled runtime dependency: {name}')
    for name in ('profiles','ui-settings.json','config.json','config.queue-backup.json'):
        if (directory/name).exists() or (internal/name).exists():raise ValueError(f'User data included in bundle: {name}')
    with pefile.PE(str(directory/'MetinBot.exe')) as image:
        if image.FILE_HEADER.Machine!=0x8664:raise ValueError('Executable must be Windows x64.')
        manifests=[]
        for kind in image.DIRECTORY_ENTRY_RESOURCE.entries:
            if kind.id!=24:continue
            for resource in kind.directory.entries:
                for language in resource.directory.entries:
                    value=language.data.struct
                    manifests.append(ET.fromstring(image.get_data(value.OffsetToData,value.Size)))
        levels=[node.attrib.get('level') for manifest in manifests for node in manifest.iter()
                if node.tag.endswith('requestedExecutionLevel')]
        if levels!=['requireAdministrator']:raise ValueError('Missing native administrator manifest.')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory',type=Path,required=True)
    parser.add_argument('--version',required=True);parser.add_argument('--commit',required=True)
    args=parser.parse_args();verify_bundle(args.directory,args.version,args.commit)
    print('Bundle PASS: x64, identity, dependencies, no user data, native administrator manifest.')
