"""Generate JSON schema boundary cases using the reference validator."""
import copy
import json
import sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from metin_bot.config import NUMBER_RULES, Profile, BuffSkill

base = json.loads((ROOT / 'tests/parity/profiles/calibrated.json').read_text(encoding='utf-8'))
base['buffs']['skills'] = [BuffSkill('F1').__dict__.copy()]
cases = []
def case(label, data):
    try:
        profile = Profile.from_dict(data)
        expected = profile.to_dict()
    except (ValueError, TypeError, KeyError, AttributeError):
        expected = None
    cases.append(dict(label=label, input=data, expected=expected))

paths = []
for section in ['color', 'panel', 'timing', 'buffs']:
    paths.extend(([section, name], rule) for name, rule in NUMBER_RULES.items() if name in base[section])
paths.extend((['buffs', 'skills', 0, name], NUMBER_RULES[name]) for name in ['wait_seconds', 'interval_seconds'])
for path, rule in paths:
    for value in [None, True, False, '1', [], {}, -1, 0, rule.low, rule.high, rule.low - 0.01, rule.high + 0.01, (rule.low + rule.high) / 2]:
        data = copy.deepcopy(base)
        node = data
        for key in path[:-1]: node = node[key]
        node[path[-1]] = value
        case('.'.join(map(str, path)) + '=' + repr(value), data)
for section in ['color', 'panel', 'timing', 'buffs']:
    data = copy.deepcopy(base)
    data[section]['unexpected'] = 1
    case(section + '.unknown', data)
for field in ['id', 'name']:
    data = copy.deepcopy(base)
    del data[field]
    case('missing.' + field, data)
for value in ['', 'F7', 'F8', 'Ctrl+F8', 'Ctrl+G', 'Ctrl+Shift+F1', 'Alt+A', 'Ctrl+Ctrl+A', 'Delete']:
    data = copy.deepcopy(base)
    data['buffs']['skills'][0]['shortcut'] = value
    case('shortcut.' + value, data)
out = ROOT / 'tests/parity/profiles/schema-cases.json'
out.write_text(json.dumps(cases, ensure_ascii=False, indent=2, allow_nan=False) + '\n', encoding='utf-8')
print(f'{len(cases)} profile schema boundary cases')
