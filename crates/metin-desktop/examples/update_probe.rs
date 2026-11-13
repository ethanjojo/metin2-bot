//! Read-only public-feed acceptance. Optional download verifies and discards; never installs.
use clap::Parser;
use metin_desktop::updates::{UpdateChannel, UpdateService, UpdateState};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Parser)]
struct Args {
    report: PathBuf,
    #[arg(long)]
    download: bool,
    #[arg(long, default_value = "0.0.0")]
    current_version: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        report,
        download,
        current_version,
    } = Args::parse();
    let mut service = UpdateService::new(&current_version)?;
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
        if let Some(mut value) = result {
            value["current_version"] = current_version.clone().into();
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
