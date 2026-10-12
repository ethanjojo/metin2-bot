"""Exercise real editor widgets without sending input or saving user profiles."""
from pathlib import Path
import runpy
import tkinter as tk
from unittest.mock import patch
from metin_bot.config import BuffSkill, Profile

with patch.object(tk.Tk, 'mainloop', lambda self: None):
    context = runpy.run_path(str(Path(__file__).with_name('ui_fixture.py')))
app, root = context['editor'], context['root']
try:
    app.buff_list.load((BuffSkill('3', interval_seconds=14400), BuffSkill('4', interval_seconds=600)))
    root.update()
    assert 'interval_seconds' not in app.buff_fields
    row = app.buff_list.rows[0]
    assert row.interval.get() == '240' and row.unit_choice.get() == 'min'
    row.unit_choice.set('s')
    row.unit_choice.event_generate('<<ComboboxSelected>>')
    root.update()
    assert row.interval.get() == '14400'
    assert app.buff_list.skills()[0].interval_seconds == 14400
    row.unit_choice.set('min')
    row.unit_choice.event_generate('<<ComboboxSelected>>')
    root.update()
    assert row.interval.get() == '240'
    app.buff_list.update_timers((120, 450), enabled=True, dirty=False)
    assert [r.timer_label.cget('text') for r in app.buff_list.rows] == ['00:02:00', '00:07:30']
    app.buff_list.move(0, 1)
    assert [s.interval_seconds for s in app.buff_list.skills()] == [600, 14400]
    row = app.buff_list.rows[0]
    row.mode.set('Activate when inactive')
    app.buff_list._mode_changed(row)
    root.update()
    assert not row.interval_box.winfo_manager()
    assert app.buff_list.skills()[0].interval_seconds is None
    assert row.state_label.cget('text') == 'Check between Metins'
    checked = 0
    for path in (Path(__file__).parent.parent / 'profiles').glob('*/profile.json'):
        import json
        data = json.loads(path.read_text(encoding='utf-8'))
        if 'id' not in data:
            continue
        profile = Profile.from_dict(data)
        old = data.get('buffs', {}).get('interval_seconds', 120)
        for source, skill in zip(data.get('buffs', {}).get('skills', []), profile.buffs.skills):
            assert skill.interval_seconds == (source.get('interval_seconds', old) if skill.mode == 'refresh' else None)
        checked += 1
    print(f'Tk editor PASS; {checked} user profiles migrated in memory; no saves or game input.')
finally:
    app.worker.cancel_event.set()
    app.worker.shutdown.set()
    app.worker.thread.join(timeout=3)
    root.destroy()
