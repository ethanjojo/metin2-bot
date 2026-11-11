//! Fixture-only detection benchmark; no screen capture or game input.
use metin_core::Profile;
use metin_vision::{detect_candidates, load_image, panel_match};
use std::{hint::black_box, path::Path, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/parity");
    let p = Profile::from_json(&std::fs::read_to_string(
        root.join("profiles/calibrated.json"),
    )?)?;
    let template = load_image(&root.join("frames/template.png"))?;
    let mut results = vec![];
    for name in ["search.png", "panel.png"] {
        let frame = load_image(&root.join("frames").join(name))?;
        for run in 1..=3 {
            let mut samples = vec![];
            for iteration in 0..110 {
                let start = Instant::now();
                black_box(detect_candidates(&frame, &p)?);
                black_box(panel_match(&frame, &template)?);
                if iteration >= 10 {
                    samples.push(start.elapsed().as_secs_f64() * 1000.);
                }
            }
            samples.sort_by(f64::total_cmp);
            results.push(serde_json::json!({"frame":name,"run":run,"samples":100,"median_ms":samples[50],"p95_ms":samples[94]}));
        }
    }
    let peak: Option<usize>;
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::{
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::GetCurrentProcess,
        };
        let mut counters = PROCESS_MEMORY_COUNTERS::default();
        // SAFETY: current-process pseudo handle and correctly sized writable struct.
        let ok = unsafe {
            GetProcessMemoryInfo(
                GetCurrentProcess(),
                &mut counters,
                std::mem::size_of_val(&counters) as u32,
            )
        };
        if ok == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        peak = Some(counters.PeakWorkingSetSize);
    }
    #[cfg(not(windows))]
    {
        peak = None;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"cases":results,"peak_working_set_bytes":peak})
        )?
    );
    Ok(())
}
