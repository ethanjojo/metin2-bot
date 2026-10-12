import unittest
from types import SimpleNamespace
from unittest.mock import Mock
from metin_bot.run_ui import build_overview, update_overview, preview_status
from metin_bot.session import Statistics


class OverviewTests(unittest.TestCase):
    def test_loading_icon_animates_inside_button_without_changing_size(self):
        import tkinter as tk
        from metin_bot.ui_controls import LoadingButton, icon
        root=tk.Tk();self.addCleanup(root.destroy)
        normal=icon('refresh')
        button=LoadingButton(root,text='Snapshot',image=normal,compound='left',width=0)
        button.pack();root.update()
        size=(button.winfo_width(),button.winfo_height())
        original=str(button.cget('image'))
        button.set_loading(True);root.update_idletasks()
        first=str(button.cget('image'))
        self.assertNotEqual(first,original)
        self.assertTrue(button.instate(['disabled']))
        self.assertEqual((button.winfo_width(),button.winfo_height()),size)
        root.after(130,root.quit);root.mainloop()
        self.assertNotEqual(str(button.cget('image')),first)
        button.set_loading(False);root.update_idletasks()
        self.assertEqual(str(button.cget('image')),original)
        self.assertTrue(button.instate(['!disabled']))
        self.assertEqual((button.winfo_width(),button.winfo_height()),size)

    def test_messages_never_change_preview_geometry(self):
        import tkinter as tk
        from tkinter import ttk
        root=tk.Tk();root.geometry('980x640');root.withdraw()
        self.addCleanup(root.destroy)
        app=SimpleNamespace(reason=tk.StringVar(root,value=''),status=tk.StringVar(root,value='Stopped'),
                            icons={},reset_statistics=Mock())
        build_overview(app,root)
        preview=ttk.Frame(root);preview.pack(fill='both',expand=True)
        from metin_bot.ui_controls import message_line
        info=tk.StringVar(root,value='')
        message_line(root,info).pack(fill='x')
        root.deiconify();root.update()
        original=(preview.winfo_y(),preview.winfo_width(),preview.winfo_height())
        for status,reason in [('Paused','Vraťte hru do popředí.'),
                              ('Stopped','Obnovena poslední úprava.'),
                              ('Error','Dlouhé vysvětlení chyby. '*40),
                              ('Stopped','První řádek\nDruhý řádek'),('Stopped','')]:
            app.status.set(status);app.reason.set(reason);info.set(reason);root.update_idletasks()
            self.assertEqual((preview.winfo_y(),preview.winfo_width(),preview.winfo_height()),original)

    def test_preview_status_never_labels_old_frame_as_live(self):
        self.assertEqual(preview_status(False,False,None,10,1),'No frame')
        self.assertEqual(preview_status(True,True,0,10,1),'Frozen frame')
        self.assertEqual(preview_status(True,False,9.5,10,1),'Live preview')
        self.assertEqual(preview_status(True,False,8,10,1),'Stale frame')
        self.assertEqual(preview_status(True,False,None,10,1),'Stale frame')
    def test_snapshot_formats_counts_and_deadline_without_own_clock(self):
        app=SimpleNamespace(run_title=Mock(),run_timer=Mock(),run_timer_caption=Mock(),
                            metric_values={k:Mock() for k in ('time','completed','clicks','failed')})
        update_overview(app,'Acquiring target',Statistics(3661,12,15,3),4.5)
        app.run_title.set.assert_called_with('Waiting for target confirmation')
        app.metric_values['time'].set.assert_called_with('01:01:01')
        app.metric_values['completed'].set.assert_called_with('12')
        app.run_timer.set.assert_called_with('4.5 s')
        app.run_timer_caption.set.assert_called_with('Acquiring target')
        update_overview(app,'Stopped',Statistics())
        app.run_title.set.assert_called_with('Stopped')
        app.run_timer.set.assert_called_with('—')
