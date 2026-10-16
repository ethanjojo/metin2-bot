# Reference parity corpus

`tests/parity/manifest.json` inventories all 256 original test IDs, their P01–P31 contracts and intended Rust replacements. A pending entry is deliberately not an assertion of completed direct coverage. Some planned file splits were consolidated into simple modules; current executable evidence is below.

| Corpus / behavior | Rust verification |
| --- | --- |
| 24 controller scenarios, 648 events including decisions, acknowledgments, mount, timers and statistics | `metin-core/tests/reference_controllers.rs` |
| 327 JSON numeric/bind/schema boundaries and normalization cases | `metin-core/tests/reference_profiles.rs` |
| Original image calls, candidate tracker events, 10 buff detectors and 84 observations | `metin-vision/tests/reference_vision.rs` |
| Synthetic candidates and panel scores | `metin-vision/tests/parity.rs` |
| Defaults, shared numeric rules, legacy intervals, half-even geometry, shortcuts | `metin-core/tests/profile.rs`; desktop numeric/keybind tests |
| Typed immutable frames and input payload/guard/cleanup behavior | `metin-platform/tests/{contract,input_executor,windows_input}.rs` |
| Store, selection, Unicode casefold, lock, safe assets; staged import and interrupted legacy recovery | `metin-desktop/tests/{storage,import}.rs` |
| Editor geometry, crops, history, viewport and calibration size invalidation | `metin-desktop/tests/editor.rs` |
| Stop generations/full queue, same-target refresh and mount uncertainty | `metin-desktop/tests/worker.rs` |
| Actual egui pointer/keyboard interaction, all tabs/themes and mount acknowledgment | `metin-desktop/src/ui/tests.rs` |
| Real Win32 child fixture (not a unit-test substitute for gameplay) | `metin-platform/examples/native-probe.rs` |

Temporary Python export scripts are reference tooling only. They are removed during T9 cutover after golden fixtures and complete accepted replacements are committed. They are not shipped in the Rust application or used at runtime.
