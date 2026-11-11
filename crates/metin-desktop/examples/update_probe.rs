//! Read-only public-feed acceptance. Optional download verifies and discards; never installs.
use metin_desktop::updates::{UpdateChannel, UpdateService, UpdateState};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let report = PathBuf::from(args.next().ok_or("Pass the output JSON path.")?);
    let download = args.next().is_some_and(|arg| arg == "--download");
    let mut service = UpdateService::new("0.0.0")?;
    service.check(UpdateChannel::Preview)?;
    let deadline = Instant::now() + Duration::from_secs(if download { 620 } else { 25 });
    loop {
        service.poll();
        let result = match service.state() {
            UpdateState::UpToDate => Some(
                serde_json::json!({"public_feed_verified":true,"compatible_new_installer":false,"download_verified":false,"installer_executed":false}),
            ),
            UpdateState::Available(candidate) if !download => Some(
                serde_json::json!({"public_feed_verified":true,"compatible_new_installer":true,"version":candidate.version(),"size":candidate.size(),"download_verified":false,"installer_executed":false}),
            ),
            UpdateState::Available(_) => {
                service.download()?;
                None
            }
            UpdateState::Ready(candidate) => Some(
                serde_json::json!({"public_feed_verified":true,"compatible_new_installer":true,"version":candidate.version(),"size":candidate.size(),"download_verified":true,"installer_executed":false}),
            ),
            UpdateState::Failed(error) => return Err(error.clone().into()),
            _ => None,
        };
        if let Some(value) = result {
            let text = serde_json::to_string_pretty(&value)?;
            std::fs::write(report, text.as_bytes())?;
            println!("{text}");
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err("Update probe timed out.".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
