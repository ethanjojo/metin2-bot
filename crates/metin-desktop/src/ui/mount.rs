use super::*;

impl MetinApp {
    pub(super) fn confirm_mount(&mut self, mounted: Option<bool>) -> bool {
        if self.editor.is_recalibrating() {
            return false;
        }
        if mounted != self.declared_mount()
            && let Some(mounted) = mounted
            && let Some(key) = self.key()
            && self.snapshot.key == Some(key)
            && self.snapshot.revision == self.revision
        {
            self.runtime.stop();
            self.revision += 1;
            self.send(Command::DeclareMount {
                key,
                mounted,
                revision: self.revision,
            });
            true
        } else {
            false
        }
    }
}
