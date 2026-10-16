# Fixture performance evidence

Host: Microsoft Windows 10.0.26300, x64, AMD64 Family 25 Model 80 Stepping 0 (AuthenticAMD), 16 logical processors. CPU marketing name was not retrieved because CIM access is unavailable in the managed shell. Rust 1.99.0 release, OpenCV 4.13.0, bindings 0.101.1; Python 3.14.0, OpenCV 4.13.0, NumPy 2.3.5. Fixture images are 800×600.

Procedure: `cargo build --release -p metin-vision --example benchmark --locked`, then `tools/benchmark-rust.ps1`. Each language runs the identical detection + panel matching workload, ten warmups followed by 100 samples, repeated three times on each of two fixtures. Raw reports: `build/rust-migration/benchmark-{python,rust,comparison}.json`. Peak working set comes from each measured process's own Windows memory counters; the Python virtualenv launcher is not mistaken for the actual Python worker.

| Workload | Python median ms, runs 1/2/3 | Rust median ms, runs 1/2/3 | Python p95 ms | Rust p95 ms |
| --- | --- | --- | --- | --- |
| search.png | 12.32 / 12.78 / 12.14 | 7.26 / 7.73 / 7.26 | 18.30 / 17.67 / 16.67 | 9.61 / 10.75 / 9.60 |
| panel.png | 12.39 / 12.07 / 11.46 | 7.85 / 7.28 / 6.32 | 19.81 / 16.29 / 15.03 | 11.78 / 10.77 / 7.85 |

Peak working set for this vision-only process: Python 54,169,600 bytes; Rust 21,770,240 bytes. No >10% regression occurred in these measured fixture cases. This is not full-bot RAM, GPU memory, capture latency, idle GUI CPU or game throughput. Those measurements remain open under T0/T9 and cannot be inferred from these results.

Preview packaging was compiled locally. Its exact final artifact sizes and hashes are recorded in each unique staging folder's SHA256SUMS and acceptance evidence; an installer build alone is not proof of clean-machine installation or upgrade.
