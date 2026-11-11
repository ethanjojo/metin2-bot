//! Bounded, read-only thumbnail work for the connection chooser.
use metin_platform::{Frame, StopToken, TargetToken};
use std::{
    sync::mpsc::{self, Receiver, SyncSender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(super) const MAX_VISIBLE_PREVIEWS: usize = 32;
const PREVIEW_TIMEOUT: Duration = Duration::from_millis(450);

struct Request {
    generation: u64,
    targets: Vec<TargetToken>,
}
pub(super) struct Preview {
    pub generation: u64,
    pub target: TargetToken,
    pub image: Result<Frame, String>,
}
pub(super) struct PreviewWorker {
    sender: Option<SyncSender<Request>>,
    receiver: Receiver<Preview>,
    stop: StopToken,
    thread: Option<JoinHandle<()>>,
}
impl PreviewWorker {
    pub fn new(context: eframe::egui::Context) -> Result<Self, String> {
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let (results, receiver) = mpsc::sync_channel(MAX_VISIBLE_PREVIEWS);
        let stop = StopToken::default();
        let thread_stop = stop.clone();
        let thread = thread::Builder::new()
            .name("window-thumbnails".into())
            .spawn(move || {
                while let Ok(request) = requests.recv() {
                    for target in request.targets.into_iter().take(MAX_VISIBLE_PREVIEWS) {
                        if thread_stop.cancelled(request.generation) {
                            break;
                        }
                        let image = metin_platform::capture_window_preview(
                            target,
                            Instant::now() + PREVIEW_TIMEOUT,
                            &thread_stop,
                            request.generation,
                        )
                        .map(|frame| thumbnail(&frame))
                        .map_err(|error| error.to_string());
                        if thread_stop.cancelled(request.generation) {
                            break;
                        }
                        // A hidden/slow UI cannot make a thumbnail worker block.
                        let _ = results.try_send(Preview {
                            generation: request.generation,
                            target,
                            image,
                        });
                        context.request_repaint();
                    }
                }
            })
            .map_err(|error| format!("Cannot start window previews: {error}"))?;
        Ok(Self {
            sender: Some(sender),
            receiver,
            stop,
            thread: Some(thread),
        })
    }
    pub fn request(&self, targets: Vec<TargetToken>) -> Result<u64, String> {
        let generation = self.stop.generation();
        self.sender
            .as_ref()
            .ok_or("Window previews are closed.")?
            .try_send(Request {
                generation,
                targets: targets.into_iter().take(MAX_VISIBLE_PREVIEWS).collect(),
            })
            .map_err(|error| error.to_string())?;
        Ok(generation)
    }
    pub fn poll(&self) -> Option<Preview> {
        self.receiver.try_recv().ok()
    }
    pub fn cancel(&self) {
        self.stop.stop();
    }
}
impl Drop for PreviewWorker {
    fn drop(&mut self) {
        self.cancel();
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) fn thumbnail(frame: &Frame) -> Frame {
    let scale = (320. / f64::from(frame.width))
        .min(180. / f64::from(frame.height))
        .min(1.);
    let width = (f64::from(frame.width) * scale).round().max(1.) as u32;
    let height = (f64::from(frame.height) * scale).round().max(1.) as u32;
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 3);
    for y in 0..height {
        for x in 0..width {
            let sx = u64::from(x) * u64::from(frame.width) / u64::from(width);
            let sy = u64::from(y) * u64::from(frame.height) / u64::from(height);
            let offset = (sy as usize * frame.width as usize + sx as usize) * 3;
            pixels.extend_from_slice(&frame.bgr[offset..offset + 3]);
        }
    }
    Frame::new(width, height, pixels).expect("Bounded thumbnail preserves BGR dimensions")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thumbnails_are_bounded_preserve_aspect_and_copy_source_pixels() {
        for (width, height, expected) in [
            (1920, 1080, [320, 180]),
            (600, 1200, [90, 180]),
            (5, 7, [5, 7]),
        ] {
            let frame = Frame::new(
                width,
                height,
                vec![37; width as usize * height as usize * 3],
            )
            .unwrap();
            let result = thumbnail(&frame);
            assert_eq!([result.width, result.height], expected);
            assert!(result.bgr.iter().all(|v| *v == 37));
        }
    }
    #[test]
    fn cancellation_invalidates_the_exact_requested_generation() {
        let stop = StopToken::default();
        let old = stop.generation();
        stop.stop();
        assert!(stop.cancelled(old));
        assert!(!stop.cancelled(stop.generation()));
    }
    #[test]
    fn preview_work_is_bounded_and_invalid_windows_return_typed_failures() {
        let worker = PreviewWorker::new(eframe::egui::Context::default()).unwrap();
        let invalid = TargetToken {
            handle: 0,
            pid: u32::MAX,
            created: 0,
        };
        let generation = worker
            .request(vec![invalid; MAX_VISIBLE_PREVIEWS + 20])
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut count = 0;
        while count < MAX_VISIBLE_PREVIEWS && Instant::now() < deadline {
            if let Some(result) = worker.poll() {
                assert_eq!(result.generation, generation);
                assert_eq!(result.target, invalid);
                assert!(result.image.is_err());
                count += 1;
            } else {
                thread::sleep(Duration::from_millis(1));
            }
        }
        assert_eq!(count, MAX_VISIBLE_PREVIEWS);
        assert!(worker.poll().is_none());
        worker.cancel();
    }
}
