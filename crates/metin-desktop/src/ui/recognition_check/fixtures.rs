use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};

#[derive(Clone, Copy, Debug)]
pub enum RecognitionCheckFixture {
    HpPositive,
    HpNegative,
    BuffActive,
    BuffInactive,
    BuffAmbiguous,
    Waiting,
    Failure,
}
impl MetinApp {
    pub fn prepare_recognition_check_fixture(
        &mut self,
        context: &egui::Context,
    ) -> Result<(), String> {
        let subject = self.recognition_fixture_subject();
        let frame = self
            .editor
            .frozen
            .as_ref()
            .or(self.snapshot.frame.as_ref())
            .ok_or("The recognition fixture requires an image.")?;
        let evaluated = evaluate(
            frame,
            &self.editor.profile,
            self.editor.template.as_ref(),
            subject,
        )?;
        let check = self
            .recognition_context(subject)
            .ok_or("The recognition fixture requires a selected synthetic window.")?;
        self.recognition_check.state = DialogState::Ready(make_report(
            context,
            check,
            Instant::now(),
            self.snapshot.frame_id,
            evaluated,
        ));
        Ok(())
    }
    fn recognition_fixture_subject(&self) -> RecognitionSubject {
        if self.actions_page == ActionsPage::Buffs && self.buff_detail {
            RecognitionSubject::Buff(self.selected_skill)
        } else {
            RecognitionSubject::HpPanel
        }
    }
    pub fn prepare_recognition_check_state_fixture(
        &mut self,
        context: &egui::Context,
        fixture: RecognitionCheckFixture,
    ) -> Result<(), String> {
        use RecognitionCheckFixture::*;
        let subject = match fixture {
            HpPositive | HpNegative => RecognitionSubject::HpPanel,
            BuffActive | BuffInactive | BuffAmbiguous => {
                RecognitionSubject::Buff(self.selected_skill)
            }
            Waiting | Failure => self.recognition_fixture_subject(),
        };
        let check = self
            .recognition_context(subject)
            .ok_or("The recognition fixture requires a selected synthetic window and subject.")?;
        if matches!(fixture, Waiting) {
            self.recognition_check.state = DialogState::Pending(PendingCheck {
                context: check,
                after_frame: self.snapshot.frame_id,
                // This screenshot fixture deliberately withholds a fresh frame for the batch.
                requested_at: Instant::now() + Duration::from_secs(60),
            });
            return Ok(());
        }
        if matches!(fixture, Failure) {
            self.recognition_check.state = DialogState::Failure(CheckFailure { context: check,
                message: "No fresh image arrived. Check that the selected game is visible and responding.".into() });
            return Ok(());
        }
        let source = self
            .editor
            .frozen
            .as_ref()
            .or(self.snapshot.frame.as_ref())
            .ok_or("The recognition fixture requires an image.")?;
        let mut profile = self.editor.profile.clone();
        let mut pixels = (0..source.bgr.len())
            .map(|index| ((index * 73 + index / 7 * 19 + 41) % 251) as u8)
            .collect::<Vec<_>>();
        match fixture {
            HpPositive => paste_reference(
                &mut pixels,
                source.width,
                source.height,
                0,
                0,
                self.editor
                    .template
                    .as_ref()
                    .ok_or("The HP fixture needs a reference.")?,
            )?,
            BuffActive | BuffInactive | BuffAmbiguous => {
                let RecognitionSubject::Buff(index) = subject else {
                    unreachable!()
                };
                let skill = profile
                    .buffs
                    .skills
                    .get_mut(index)
                    .ok_or("The buff fixture needs a skill.")?;
                if matches!(fixture, BuffAmbiguous) {
                    skill.inactive_icon = skill.icon.clone();
                } else {
                    skill.icon = Some(distinct_fixture_icon(
                        skill
                            .icon
                            .as_ref()
                            .ok_or("The buff fixture needs an Active reference.")?,
                        false,
                    )?);
                    skill.inactive_icon = Some(distinct_fixture_icon(
                        skill
                            .inactive_icon
                            .as_ref()
                            .ok_or("The buff fixture needs an Inactive reference.")?,
                        true,
                    )?);
                }
                let icon = if matches!(fixture, BuffInactive) {
                    skill.inactive_icon.as_ref()
                } else {
                    skill.icon.as_ref()
                }
                .ok_or("The buff fixture needs the selected appearance reference.")?;
                let icon =
                    metin_vision::decode_png(&icon.png_bytes().map_err(|error| error.to_string())?)
                        .map_err(|error| error.to_string())?;
                let [left, top, _, _] = profile
                    .buffs
                    .icon_region
                    .ok_or("The buff fixture needs a detection area.")?
                    .pixels(source.width, source.height);
                paste_reference(&mut pixels, source.width, source.height, left, top, &icon)?;
            }
            HpNegative => {}
            Waiting | Failure => unreachable!(),
        }
        let frame = metin_platform::Frame::new(source.width, source.height, pixels)
            .map_err(|error| error.to_string())?;
        let evaluated = evaluate(&frame, &profile, self.editor.template.as_ref(), subject)?;
        self.recognition_check.state = DialogState::Ready(make_report(
            context,
            check,
            Instant::now(),
            self.snapshot.frame_id,
            evaluated,
        ));
        Ok(())
    }
}
fn distinct_fixture_icon(
    original: &metin_core::BuffIcon,
    inactive: bool,
) -> Result<metin_core::BuffIcon, String> {
    let reference =
        metin_vision::decode_png(&original.png_bytes().map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let width = reference.width;
    let height = reference.height;
    let mut pixels = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            let nx = (2. * (x as f64 + 0.5) / width as f64) - 1.;
            let ny = (2. * (y as f64 + 0.5) / height as f64) - 1.;
            let stroke = if inactive {
                (nx.abs() - ny.abs()).abs() < 0.16
            } else {
                nx.abs() < 0.16 || ny.abs() < 0.16
            };
            let inside = nx.abs() < 0.8 && ny.abs() < 0.8;
            pixels.extend_from_slice(if stroke && inside {
                if inactive {
                    &[150, 180, 210]
                } else {
                    &[80, 210, 120]
                }
            } else {
                &[17, 22, 28]
            });
        }
    }
    let frame =
        metin_platform::Frame::new(width, height, pixels).map_err(|error| error.to_string())?;
    Ok(metin_core::BuffIcon {
        png: STANDARD.encode(metin_vision::encode_png(&frame).map_err(|error| error.to_string())?),
        region: original.region,
    })
}
fn paste_reference(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    left: u32,
    top: u32,
    reference: &metin_platform::Frame,
) -> Result<(), String> {
    if left + reference.width > width || top + reference.height > height {
        return Err("The fixture reference does not fit the client image.".into());
    }
    for y in 0..reference.height {
        let from = (y * reference.width * 3) as usize;
        let to = (((top + y) * width + left) * 3) as usize;
        let len = reference.width as usize * 3;
        pixels[to..to + len].copy_from_slice(&reference.bgr[from..from + len]);
    }
    Ok(())
}
