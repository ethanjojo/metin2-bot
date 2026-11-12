use super::*;

impl MetinApp {
    pub(super) fn remember_latest_panel_position(&mut self) {
        if let Some((target, offset)) = self
            .overlay
            .as_ref()
            .and_then(|overlay| overlay.panel_position())
        {
            self.remember_panel_position(target, offset);
        }
    }
    pub(super) fn remember_panel_position(
        &mut self,
        target: TargetToken,
        offset: crate::placement::PanelOffset,
    ) {
        if self.target != Some(target) || self.window_chooser.open {
            return;
        }
        let Some((key_target, key)) = &self.placement_key else {
            return;
        };
        if *key_target != target {
            return;
        }
        if crate::placement::remember_position(
            &mut self.settings.panel_positions,
            key.clone(),
            offset,
        ) {
            self.placement_save_due =
                Some(std::time::Instant::now() + std::time::Duration::from_millis(500));
        }
    }
    pub(super) fn flush_panel_position(&mut self, immediate: bool) {
        if self
            .placement_save_due
            .is_some_and(|due| immediate || std::time::Instant::now() >= due)
        {
            match self.settings.save(&self.root) {
                Ok(()) => self.placement_save_due = None,
                Err(error) => {
                    self.placement_save_due = None;
                    self.error = Some(format!("Could not save panel position: {error}"));
                }
            }
        }
    }
}
