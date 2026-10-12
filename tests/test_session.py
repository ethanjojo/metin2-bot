import unittest
from metin_bot.session import Session, duration


class SessionTests(unittest.TestCase):
    def test_pause_excludes_idle_time_and_snapshot_is_independent(self):
        s=Session();s.advance(10,True);s.advance(13,True)
        before=s.snapshot();s.pause();s.advance(100,False);s.advance(101,True);s.advance(105,True)
        self.assertEqual(before.active_seconds,3)
        self.assertEqual(s.snapshot().active_seconds,7)
        s.reset();self.assertEqual(s.snapshot().active_seconds,0)

    def test_duration_handles_hours_and_negative(self):
        self.assertEqual(duration(3661),'01:01:01')
        self.assertEqual(duration(-1),'00:00:00')

    def test_pause_accounts_for_last_active_interval(self):
        s=Session();s.advance(10,True);s.pause(12.5);s.advance(100,False)
        self.assertEqual(s.snapshot().active_seconds,2.5)
