# MetinBot Rust + egui Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Tento dokument zadává postupnou implementaci; nevytváří oprávnění ke spuštění přepisu nebo vydání.

**Goal:** Nahradit Python aplikaci samostatnou Rust + egui aplikací se všemi současnými funkcemi na Windows a připravenými, postupně ověřovanými backendy pro Linux a macOS.

**Architecture:** Čtyři crates: metin-core (model/rozhodování), metin-platform (OS hranice), metin-vision (OpenCV), metin-desktop (egui, worker, storage). Jediný worker vlastní běhový stav; GUI posílá příkazy a zobrazuje immutable snapshoty. Nativní OS transporty jsou oddělené od společné logiky.

**Tech Stack:** Rust stable připnutý po build probe, egui/eframe + wgpu, OpenCV Rust bindings, serde/serde_json, clap, uuid, base64, thiserror, tracing + tracing-subscriber; platformní dependencies pouze pro příslušný target. Standardní vlákna a bounded kanály, žádný Tokio v core ani dodatečný web frontend. Konkrétní dependency verze a volbu lock/native-dialog knihovny uzavře T1.

**Spec:** [Návrh, kontrakty, matice P01–P31](../specs/2026-10-16-rust-egui-migration-design.md).

Stav: uživatel schválil realizaci 2026-10-16; Rust implementace a lokální Windows verifikace probíhají na `codex/rust-egui-migration`. Průběžné důkazy a zbývající gates jsou v [progress.md](../../rust/progress.md) a [acceptance.md](../../rust/acceptance.md). Baseline `54ac124`; úplná M1 ani platformní M2–M4 dosud nejsou prohlášeny za splněné.

## Global Constraints

- M1 = úplná Windows parita; Linux/macOS build a GUI nejsou tvrzením funkční herní automatizace.
- M2 = X11, M3 = macOS, M4 = konkrétní Wayland prostředí; každý backend má vlastní gameplay gate.
- P01–P31 specifikace jsou povinná matice funkční parity; T0 ji rozšíří o úplný inventář testů.
- UI a dokumentace pro uživatele zůstanou anglicky; vlastní názvy se zachovají.
- Jeden executable `metinbot`, žádný Python runtime, žádné skryté OS/input fallbacky.
- Prodlevy input executor čekají přerušitelně po nejvýše 20 ms.
- Zachovat současný JSON formát při M1; `process_name` je hint/filtr, nikoli identita cíle.
- Žádná platforma nesmí potichu vypustit kontroly P09–P11.
- Python a Rust nesmějí současně zapisovat do kanonického user-data kořene.
- OpenCV zůstává jediná detekční implementace; přesné verze se připnou v T1.
- Publikace vyžaduje samostatný pokyn k vydání, není součástí této přípravy plánu.

## Review Focus

- Stop při plné frontě, probíhajícím stisku, současném F7/F8 a zavírání: musí zrušit i odložený start a uvolnit pouze vlastní vstupy (T4/T5).
- Reused handle, opožděný snapshot/ack a DPI změna mezi capture a akcí: nikdy nevstoupit do jiného cíle (T3/T4/T5).
- Save/rename/reorder při čekajícím skillu a změna mount deklarace: správná identita, timer a snapshot revision (T5/T7).
- Načtení přeneseného profilu, neúplný PNG, filesystem chyba nebo import po přerušení: původní data zůstanou použitelná (T2).
- Povolení odebrané během běhu nebo Wayland session bez ověřitelného focus: bezpečné ukončení, pravdivý důvod, žádná obcházející cesta (T8/T10/T11/T12).

## Pořadí a review gates

`T0 -> T1 -> T2 -> T3 -> T4 -> T5 -> T6 -> T7 -> T8 -> T9` tvoří M1. `T10` (X11), `T11` (macOS) a `T12` (Wayland) navazují na společné kontrakty a mají samostatné release gates. Jsou možné dříve po T5 pro ověření architektury, nesmějí však nahrazovat nedokončenou Windows paritu. Postupovat v malých, ověřitelných commitech; commit příkazy nejsou autorizací push/merge/release.

Každý úkol: nejprve přidat uvedené behavioral testy a vidět očekávané selhání kvůli chybějícímu chování, potom implementovat, spustit testy a zapsat důkaz. Compile error při první neexistující API je přípustný red krok, finální green musí testovat skutečné chování. Dokumentační inventář T0 nepotřebuje umělé testy.

## T0 — Zmrazit referenci a vytvořit úplnou mapu parity

**Files:** vytvořit `tests/parity/manifest.json`, `tests/parity/profiles/`, `tests/parity/frames/`, `tests/parity/traces/`, `docs/rust/parity.md`, `docs/rust/performance.md`, dočasný `tools/export_parity.py`.

**Interfaces:** vstup = dnešní Python moduly/testy; výstup = verzovaný manifest s baseline commit/dependencies, každým test ID, P-ID, očekávanými výsledky a cílovým Rust testem. Replay obsahuje monotónní čas, pozorování, command/ack, akce, mount, timery a statistiky; ne HWND, osobní cesty ani wall-clock závislosti.

- [ ] Zkontrolovat nové AGENTS instrukce a diff vůči `54ac124`; inventář aktualizovat, pokud se změnil produkt. Nevycházet ze zastaralé kopie tohoto seznamu.
- [ ] Spustit `.\.venv\Scripts\python.exe -m unittest discover -s tests -v`, `-m compileall -q metin_bot tools tests packaging_entry.py` a `-m pip check`; uložit skutečný výsledek. Každý z 256 nalezených testů musí mít mapování nebo zdůvodněnou náhradu; žádný „vše PASS“ bez výstupu.
- [ ] Exportovat fixtures pro HSV wrap, mask boundaries, panel, hotbar active/inactive/ambiguous, stale/black frame, long skill intervals, full target–buff–mount–next target a cancellation. Použít existující `tests/data` a syntetické snímky, nikoli nevyžádané kopie uživatelských profilů.
- [ ] Zapsat baseline defaulty a všechny NUMBER_RULES z `config.py` do fixture; včetně nečíselných hodnot, NaN, chybějících/unknown polí a Python half-even zaokrouhlení.
- [ ] Změřit 3 opakování stejného zaznamenaného workloadu: capture/detection/runtime p50/p95, CPU, peak RAM, idle redraw/CPU, startup a velikost balíku. Zapsat HW/OS/resolution/build a přesný měřicí postup; živou hru používat až v explicitním acceptance scénáři.
- [ ] Self-review parity: žádný test se neztratil, všechny P01–P31 mají plánovaný důkaz. Commit `docs: establish Rust migration parity baseline`.

**Gate:** reprodukovatelný baseline; existující failures odlišeny od regresí přepisu. Bez baseline se výkon ani shoda neprohlašují.

## T1 — Ověřit Rust/egui/OpenCV build na třech OS

**Files:** root `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`; `crates/{metin-core,metin-platform,metin-vision,metin-desktop}/Cargo.toml`, jejich `src/lib.rs`; desktop `src/{main,cli,diagnostics,self_test}.rs`; `.github/workflows/rust-ci.yml`; `docs/rust/platform-support.md`.

**Interfaces:** CLI `Cli::parse` zachová přepínače P28 a exit codes; `SelfTestReport { build, checks, errors }` je strojově čitelný JSON. Tento první self-test ověří jen dosud existující komponenty, ne gameplay. Root workspace je jediný Rust build vstup.

- [ ] Přidat `crates/metin-desktop/tests/cli.rs`: `self_test_requires_report`, `report_requires_self_test`, `self_test_rejects_data_import_log_overrides`, `version_contains_build_identity`. Ověřit red přes `cargo test -p metin-desktop --test cli`.
- [ ] Založit minimální crates a CLI; GUI probe vykreslí lokální testovací texturu a region v egui bez herního inputu. OpenCV probe načte fixture PNG a vykoná `cvtColor`/`matchTemplate`.
- [ ] Nativně sestavit probe na Windows x64, Linux x64 a macOS arm64. Zvolit a připnout konkrétní toolchain, egui/eframe a OpenCV/bindings; dokumentovat MSVC/Clang, Linux dev knihovny a Apple SDK/min deployment target. Podporované minimální OS verze vycházejí z tohoto výsledku, nikoli odhadu.
- [ ] Na všech třech hostech spustit workspace fmt/clippy/test/release build z části Verifikace. Linux GUI smoke běží s display session/Xvfb, macOS/Windows screenshot probe v desktop session; headless build nepovažovat za vizuální ověření.
- [ ] Doplnit lock/native dialog dependencies podle skutečných needs; OpenCV jen potřebné moduly. Uložit build/link dependency seznam; commit `build: establish portable Rust egui workspace`.

**Gate:** stejné fixture se načtou, GUI probe se zobrazí, runtime nemá závislost na Pythonu. Pokud kombinace knihoven selže, vyřešit konkrétní build problém před pokračováním; nezavést druhou vision implementaci.

## T2 — Typovaný model, validace, profily a bezpečný import

**Files:** core `src/{profile,validation,geometry,shortcut}.rs`; desktop `src/storage/{mod,profiles,preferences,paths,import,migration}.rs`, `src/bootstrap.rs`; testy `metin-core/tests/profile.rs`, desktop `tests/{storage,import,paths}.rs`.

**Interfaces:** `Profile::validate() -> Result<(), ValidationError>`, `Profile::ready() -> bool`; `Shortcut::parse(&str) -> Result<Shortcut, ValidationError>` s OS-nezávislým Key/Modifiers. `ProfileStore::{load(Uuid),list(),save(&Profile, Option<&TemplateAsset>),select(Uuid),selected()}` vrací typed Result. `prepare_data_directory(&StorageOptions) -> Result<DataRoot, StorageError>` a `import_data(source: &Path, destination: &Path) -> Result<ImportReport, StorageError>` jsou jediná storage/import cesta.

- [ ] Red testy: `legacy_roundtrip_preserves_all_fields_and_png`, `normalizes_global_buff_interval_only_for_refresh`, `rejects_unknown_keys_nan_wrong_types_and_reserved_binds`, `half_even_coordinates_match_python`, `unicode_profile_sort_matches_baseline`, `incomplete_profile_saves_but_is_not_ready`.
- [ ] Implementovat všechny NUMBER_RULES jednou, z nich vystavit numeric UI metadata. Zachovat P26 a stávající defaults; UI draft prázdný skill řádek není neplatný uložený skill. process_name rozšířit pouze na bezpečný basename, zachovat `.exe` existujících dat.
- [ ] Red testy storage: `failed_json_commit_preserves_old_asset`, `import_rejects_nonempty_destination`, `interrupted_import_preserves_source_and_retries`, `symlink_junction_and_template_escape_rejected`, `png_header_limits_before_decode`, `second_writer_rejected` a `explicit_data_dir_disables_discovery`.
- [ ] Implementovat validovaný staging import a atomické file replacement pro každý OS, nikoli Unix rename předpoklad na Windows. Selection a UISettings samostatně; žádné přesunutí last_mounted do Profile. Při importu převádět legacy config pouze ve staging kopii; při spuštění nad existujícím kanonickým kořenem zachovat původní idempotentní migraci s atomickými zápisy a backupem. Obě cesty používají jeden společný převod.
- [ ] Spustit `cargo test -p metin-core --test profile` a `cargo test -p metin-desktop --test storage --test import --test paths`; porovnat fixture s manifestem T0. Commit `feat: preserve profiles and validated cross-platform storage`.

**Gate:** všechny podporované staré profily jdou načíst se stejným významem, vadný import nepřepíše originál/cíl; data mají jediný kanonický kořen.

## T3 — Přenést obrazové algoritmy beze změny rozhodnutí

**Files:** platform `src/frame.rs`; vision `src/{detector,tracking,panel,buffs,templates}.rs`; vision `tests/{parity,invalid_frames}.rs`.

**Interfaces:** `CapturedFrame` dle specifikace; `Vision::new(&Profile, &ProfileAssets) -> Result<Vision, VisionError>`; `Vision::observe(&CapturedFrame) -> Result<Observation, VisionError>`; `Vision::reset()`. Observation je core typ s fresh/frame provenance, kandidáty, panelem a Presence každého skillu. Templates se dekódují při configure, ne každý frame.

- [ ] Red `golden_candidates_and_order_match`, `panel_scores_match_within_1e_5`, `threshold_decisions_match`, `tracking_rejects_duplicate_and_stale_frames`, `black_or_wrong_size_is_not_absent`, `hotbar_requires_margin_and_three_fresh_samples`.
- [ ] Přenést algoritmy přes OpenCV podle T0; HSV int16 mezivýpočty, stejná morfologie/connectivity/centroid-to-selected-pixel, přesné ROI inkluzivní hranice. Active-only buff větev i dvě templates musí odpovídat baseline.
- [ ] Implementovat validaci BGR frame/stride/size/epoch, PNG validation a finite match score; žádná práce s neověřeným bufferem.
- [ ] Spustit `cargo test -p metin-vision --test parity --test invalid_frames`; skóre a rozhodnutí porovnat na všech native hostech, ne pouze Windows. Commit `feat: port deterministic vision and buff recognition`.

**Gate:** každý P04–P06/P21 replay projde; rozdíly u prahů nejsou skryté float tolerancí.

## T4 — Windows backend a jednotný bezpečný input executor

**Files:** platform `src/{contract,capabilities,error}.rs`, `src/windows/{mod,capture,input,windows,permissions,hotkeys}.rs`; desktop `src/runtime/input_executor.rs`; platform `tests/{contract,windows_input}.rs`, desktop `tests/input_executor.rs`.

**Interfaces:** `PlatformBackend` a všechny typy podle specifikace; `InputExecutor::execute(action: &Action, backend: &mut dyn PlatformBackend, stop: &StopToken) -> ActionResult`. Backend vlastní transportní key ownership; executor vlastní sekvenci a guard policy. `release_owned` je idempotentní. OS callback může atomicky zastavit a probudit worker i při zaplněné command frontě.

- [ ] Red testy skutečných transport payloadů na fake Win32 API: správný WM_KEYDOWN/UP vs WM_SYSKEYDOWN/UP, scan/context/release bits, mouse client coords, key-before-modifier release, queued event není client acknowledgement.
- [ ] Red `reused_handle_never_receives_release`, `focus_loss_releases_original_target`, `manual_mouse_movement_aborts`, `obstructed_point_aborts`, `dpi_epoch_change_aborts`, `permission_error_prevents_down`, `capture_timeout_exits_without_held_input`.
- [ ] Implementovat Win32 enumeraci a identitu, per-monitor DPI, GDI capture stejné viditelné client oblasti, kontroly před/po capture a black frame odmítnutí. Zachovat dnešní transport, žádný automatický SendInput keyboard fallback.
- [ ] Zajistit event/message pump pro F7/F8, počáteční kontrolu fyzicky držených kláves, krátké cancellable waits, explicitní cleanup a chyby při release. Snímek nesmí blokovat zpracování stop tokenu neomezeně.
- [ ] Spustit `cargo test -p metin-platform --test contract --test windows_input` a `cargo test -p metin-desktop --test input_executor` na Windows; lokální testovací okno zaznamená přijaté message pořadí. Žádné vstupy do skutečné hry v unit tests. Commit `feat: preserve guarded Windows capture and input`.

**Gate:** P09–P11 a P29 behaviorální testy; OS přijetí vstupu neoznačovat za skutečnou herní reakci.

## T5 — Controller, buffy, čas, statistiky a worker

**Files:** core `src/{controller,buffs,session,time}.rs`; desktop `src/runtime/{mod,worker,commands,snapshot}.rs`; core `tests/{controller,buffs,session}.rs`, desktop `tests/{runtime,worker}.rs`.

**Interfaces:** `Controller::step/ack`, `RuntimeHandle` a command/snapshot typy přesně podle specifikace. `Clock::now() -> MonoTime` umožňuje fake clock. `SessionKey` rozhoduje zachování statistik; execution config porovnání bez display jmen rozhoduje reset buff cyklu. Command revision neznamená novou herní session.

- [ ] Red core trace testy pro P07–P09, P19, P22–P25: kompletní target/buff/remount cyklus, zmizení panelu, panel při buffování, timeouty, prázdné hledání/rotace, unknown mount a active/unknown toggle. Čtyřhodinový timer ověřit fake časem bez čekání.
- [ ] Implementovat tagged stavy/akce, explicitní pending ActionId a ack; posun timeru až po input + wait; jednou použitý skill až po dalším target pokusu. Statistiky nesmějí resetovat buff deadlines.
- [ ] Red worker testy `stop_bypasses_full_queue`, `f8_wins_over_f7`, `stop_invalidates_pending_arm`, `old_ack_cannot_change_new_session`, `save_refresh_preserves_mount_and_timers`, `rename_does_not_reset_buffs`, `new_target_resets_session`, `close_during_input_releases_and_joins`.
- [ ] Implementovat worker ownership, latest-only preview, generation-bound příkazy, pauzu při focus/geometry/capture error a 3 s/0,5 s/12 s start gate. Po detach není F7 oprávněním ovládat dřívější cíl.
- [ ] Spustit core tři integration test targets a `cargo test -p metin-desktop --test runtime --test worker`; porovnat replay trace pole po poli s T0. Commit `feat: port bot lifecycle buff scheduling and runtime ownership`.

**Gate:** žádná business logika timerů/controlleru v GUI; všechny trace P07–P11/P19/P22–P25 shodné s baseline.

## T6 — Kalibrace a editor v egui

**Files:** desktop `src/editor/{mod,calibration,gesture,viewport,history}.rs`, `src/ui/{preview,calibration}.rs`; desktop `tests/{editor,viewport,history}.rs`.

**Interfaces:** `EditorState` vlastní draft Profile + assets + frozen frame; `apply(EditCommand) -> Result<EditOutcome, ValidationError>`. `Viewport::{frame_to_ui,ui_to_frame}` jsou jediné obousměrné transformace. `History` má 32 immutable stavů. `PreviewModel` odlišuje Frozen/Live/Stale a provenance, není duplicitním controllerem.

- [ ] Red testy P12–P15/P20: handles před interiérem, clamp/move/resize crossing, delete jedné masky, panel containment invalidace template, cancel gesture bez změny, undo template snapshot, redo branch discard, 32 state limit, padding reject, zoom point invariance a 800% limit.
- [ ] Implementovat čistou geometrii a history, potom egui interaction/painter. Crop výběr používá fyzické frame pixely i při 125/150/200 % DPI a Retina; input preview nevytváří runtime akce.
- [ ] Připojit snapshot/color/panel/active/inactive/reuse crop postupy; live snímek nesmí nahradit frozen frame během drag. Drag překresluje vektory, neprovádí OpenCV detekci v každém UI frame.
- [ ] Spustit `cargo test -p metin-desktop --test editor --test viewport --test history`; v izolovaném GUI projít drag/resize/delete/cancel/zoom/pan/undo/redo a screenshot při malém okně i HiDPI. Commit `feat: implement egui calibration editor with full editing parity`.

**Gate:** reálný editor lze ovládat pro všechny oblasti a templates, ne pouze statický mockup.

## T7 — Dokončit profily, buff UI, settings a přehled běhu

**Files:** desktop `src/ui/{mod,shell,profiles,buffs,settings,overview,keybind,numeric,theme,permissions}.rs`; desktop `tests/{ui_state,keybind,numeric}.rs`; `docs/rust/acceptance.md`.

**Interfaces:** `MetinApp` skládá EditorState, ProfileStore a RuntimeHandle. Numeric pole používá `ValidationRules` z T2. Keybind capture vrací `Option<Shortcut>`/cancel, nikoli Windows VK; runtime mount declaration dostává SessionKey + revision a UI aplikuje odpovídající ack.

- [ ] Red `keybind_focus_loss_cancels`, `reserved_f7_f8_rejected`, `czech_shift_digit_preserves_key`, `captured_ctrl_z_does_not_undo_editor`, `mount_bind_cannot_clear`, `numeric_steps_match_validation`, `display_unit_switch_preserves_seconds`.
- [ ] Red `mount_change_without_save_pauses_and_acks`, `stale_mount_ack_ignored`, `skill_reorder_moves_templates_and_timing`, `stat_reset_preserves_timer`, `saving_draft_does_not_arm`, `start_saves_then_arms`, `import_cancel_does_not_create_data`.
- [ ] Implementovat všechny operace P01/P08/P15–P20/P23/P25/P27. Samostatná UISettings palette pro oba motivy, Windows title chrome a embedded font; tooltip/overview nezamlčí plný důvod pauzy.
- [ ] Spustit `cargo test -p metin-desktop --test ui_state --test keybind --test numeric`; ručně ověřit focus/tab order, sidebar scroll/hide, oba motivy, miniaturní templates, timer countdown a anglické labely; screenshoty uložit pouze do izolované acceptance evidence.
- [ ] Vyplnit všechny P-ID v parity.md odkazy na Rust test/scénář; identifikovat dosud neověřenou část. Commit `feat: complete desktop controls and current feature parity`.

**Gate:** každá uživatelská operace dostupná v původní aplikaci má funkční odpovídající ovládání; testy nemusí kopírovat Tk internals.

## T8 — Ověřit přenositelnost a pravdivé capability stavy před M1

**Files:** platform `src/linux/{mod,x11,wayland}.rs`, `src/macos/mod.rs`, `src/capabilities.rs`; desktop `src/ui/permissions.rs`; platform `tests/capabilities.rs`, desktop `tests/platform_unavailable.rs`; `docs/rust/platform-support.md`.

**Interfaces:** build target vybere skutečný platformní backend; dosud neimplementovaný backend vrátí explicitní `Unavailable(reason)`. Runtime gate vyžaduje capture, target identity, foreground, obstruction guard, cursor/manual-input guard, input a spolehlivý stop. Nikde se nevyrábí fake WindowsAPI.

- [ ] Red `missing_guard_disables_start_but_editor_works`, `permission_revoked_cancels_session`, `portal_disconnect_invalidates_target_and_frames`, `capability_report_never_claims_gameplay_acceptance`.
- [ ] Ověřit, že core/storage/vision/UI nekódují `.exe`, HWND, VK nebo Windows cesty jako univerzální podmínku. `cfg` pouze na OS hranici; Windows specifické zdroje/testy oddělit přes target gates.
- [ ] Spustit plnou workspace verifikaci a GUI/editor/profile fixture na Linux x64 a macOS arm64; ověřit jejich data paths, práva, Unicode a HiDPI. Bez capture backendu Start jasně hlásí nedostupnost.
- [ ] V support matrix uvést Build/GUI/Capture/Input/Guards/Gameplay/Package samostatně pro každý OS/backend. Commit `feat: expose verified platform capabilities without unsafe fallbacks`.

**Gate:** M1 je architektonicky přenositelné, jiné OS nejsou falešně označené za funkční herní podporu.

## T9 — Windows acceptance, distribuce a čistý cutover

**Files:** desktop `src/{self_test,diagnostics,cli}.rs`; `tools/{build-rust.ps1,verify-rust-installer.ps1}`, `packaging/installer.iss`, `.github/workflows/release.yml`, `README.md`, `docs/rust/{parity,acceptance,performance,platform-support}.md`, nové build/package testy v desktop `tests/{self_test,release}.rs`.

**Interfaces:** zachované CLI P28; self-test testuje OpenCV, storage, obě témata a GPU/UI zvlášť s popsaným důvodem případného headless omezení, vždy izolovaná data a žádný game input. Metadata = version/build/commit/target. Installer ani release metadata nejsou závislé na Python skriptu.

- [ ] Red `self_test_never_attaches_or_sends_input`, `self_test_writes_component_failures`, `metadata_matches_artifact`, `invalid_release_ref_rejected`, `upgrade_preserves_data_and_removes_old_bundle`.
- [ ] Rozšířit self-test, capture/permissions diagnostics a sestavit Inno installer s nativními DLL/licencemi. Na Windows zachovat UAC manifest a ověřit interactive cancel/start; nepřebírat závěr z elevated CI runneru.
- [ ] Ve skutečném klientu projít P01–P31: alespoň dva potvrzené target cykly, refresh a inactive buff, dismount/remount, pause/resume, reset timeru/statistik, focus loss, F8 během action, nový profil/okno, změna rozměrů/DPI, shutdown. Evidovat OS/client/resolution/calibration/build, screenshoty a log akcí; neověřený skill/mount neoznačit za PASS.
- [ ] Opakovat T0 výkonový workload 3x v release buildu; nepřijmout nevysvětlenou >10% regresi p95 nebo peak RAM. Prověřit idle UI redraw a fronty snímků.
- [ ] Ověřit install/self-test/upgrade/uninstall na čistém Windows bez Pythonu a bez vývojářského OpenCV v PATH. Zkontrolovat, že odstranit starý `_internal` lze bez zásahu do uživatelských dat. Před kanonickým cutoverem vytvořit backup a zavřít Python aplikaci.
- [ ] Až po všech gates přepnout launcher/README/release workflow na Rust a odstranit `metin_bot/`, `packaging_entry.py`, `requirements*.txt`, PyInstaller spec/environment, nahrazené `run.cmd`/Python launch/build/check scripts, Python runtime testy a dočasný export_parity.py. Zachovat potřebný Inno setup compiler verification, publish/tag/checksum safeguards přepsané bez Pythonu, golden fixtures a import dat. Inventář T0 musí uvádět konkrétní replacement pro každý mazaný kontrakt; neponechat mrtvé odkazy/deps.
- [ ] Spustit plnou Verifikaci, vyhledat zbylé odkazy `metin_bot`, `PyInstaller`, `python.exe`, `.venv` a klasifikovat historické docs vs aktivní build. Commit `feat: replace Python distribution with verified Rust desktop app`.

**Gate M1:** P01–P31 doloženo, artefakt funguje bez Pythonu, Windows upgrade zachoval data, Linux/macOS build/editor ověřen. Remote release až při pokynu: ověřit tag -> commit -> CI -> stažený installer -> SHA256, ne pouze lokální build.

## T10 — Linux X11 backend a ověření Wine klienta (M2)

**Files:** platform `src/linux/x11.rs` rozdělit na `linux/x11/{mod,capture,input,windows,hotkeys}.rs` až s reálným objemem; platform `tests/x11.rs`; `tools/package-linux.sh`; `packaging/linux/metinbot.desktop`; `docs/rust/{platform-support,acceptance}.md`.

**Interfaces:** stejný PlatformBackend, explicitní X11 session a XTEST transport, žádná změna core action API. Input release je globální; backend musí odlišit vlastní a fyzicky držené klávesy a nesmí po focus loss poslat další down.

- [ ] Red testy X11 geometry/capture transformů, XID/PID reuse, foreground změny mezi akcemi, modifier cleanup, native keymap/CZ bindy a obsazené F7/F8. Globální transport není považován za Win32 message parity bez runtime důkazu.
- [ ] Implementovat capture klientské oblasti, ověření cíle, hit-test/occlusion, XTEST a hotkeys; lost target/permissions/session stopne worker. Xvfb test app zaznamená skutečně přijaté události.
- [ ] Spustit workspace verifikaci na native Linuxu a interaktivní scénáře P02/P08–P11/P18/P22/P23 na konkrétním Wine klientu. Zaznamenat Wine verzi, desktop, X11, DPI a input reakci hry.
- [ ] Zabalit binary+potřebné OpenCV libs+licence a desktop entry; čistý Linux bez dev knihoven ověří self-test a spuštění. Zapsat konkrétní ověřené prostředí; commit `feat: add verified Linux X11 gameplay backend`.

**Gate M2:** funkce shodné s Windows na uvedeném prostředí; globální input bez prokázaných kontrol nesmí být vydán jako supported.

## T11 — macOS backend a distribuce (M3)

**Files:** platform `src/macos/{mod,capture,input,permissions,windows,hotkeys}.rs`; platform `tests/macos.rs`; `tools/package-macos.sh`; `packaging/macos/Info.plist`; `docs/rust/{platform-support,acceptance}.md`.

**Interfaces:** PlatformBackend přes ScreenCaptureKit + AX + CoreGraphics; AppKit/permission/capture callbacks dodrží požadované main-thread hranice. Retina buffer rozměry nejsou zaměněny s points. Žádný automatický překlad herního Ctrl na Command.

- [ ] Red testy odmítnutých/odebraných oprávnění, window/owner identity, Retina transformu, focus/occlusion/manual movement guards, klávesových map, F kláves a cancellation pending capture.
- [ ] Implementovat permissions flow a konkrétní metody SDK podporované minimální macOS verzí z T1; opaque session stream identity svázat s vybraným oknem. Ověřit přijaté události v lokální test app před hrou.
- [ ] Spustit workspace verifikaci na macOS arm64; GUI obě témata, české názvy, Retina a multi-monitor. Celý gameplay scénář ověřit na skutečném kompatibilním klientu; pokud klient není dostupný, označit backend jako experimental, nikoli hotové M3.
- [ ] Sestavit .app/.dmg včetně nativních knihoven/licencí, otestovat čistý stroj a permissions po přesunu/upgrade. Signing/notarization pouze s dodanými credentials; jinak explicitně unsigned test artifact. x64 přidat až s jeho vlastním build/runtime důkazem. Commit `feat: add macOS platform backend and app packaging`.

**Gate M3:** konkrétní klient i všechny povinné guards ověřeny; podepsané doručení je samostatný důkaz od lokálního běhu.

## T12 — Wayland feasibility a podmíněný backend (M4)

**Files:** platform `src/linux/wayland.rs` případně `linux/wayland/{mod,portal,capture,input,shortcuts}.rs`; platform `tests/wayland.rs`; `docs/rust/{platform-support,acceptance}.md`.

**Interfaces:** jedna vybraná RemoteDesktop/ScreenCast portal session, PipeWire stream a GlobalShortcuts; capability assessment vrátí jednotlivě ověřitelné identity/focus/occlusion/manual movement/stop vlastnosti. Portal restore token neukládat do přenositelných profilů a nepovažovat za souhlas k automatickému inputu.

- [ ] Nejdříve ověřit na zvolené kombinaci compositor/portal backend, zda jsou všechny povinné guards vůbec dostupné. Zapsat API/metodu a důkaz pro každý guard. Pokud není prokazatelný foreground/target/obstruction/manual-input/stop kontrakt, končí tato etapa doloženým omezením; automatizace zůstane unavailable a M4 není achieved.
- [ ] Red `cancelled_portal_grants_no_input`, `revocation_stops_and_discards_old_frames`, `reconnected_stream_requires_new_target_epoch`, `missing_shortcut_binding_blocks_start`, `capture_only_is_not_automation_capability`.
- [ ] Jen pokud guard gate projde, implementovat session/capture/input/shortcuts s timeouty a user consent. Žádný root, /dev/input ani XWayland fallback; odlišit compositor-assigned trigger od preferovaného F7/F8 v UI.
- [ ] Spustit native tests a celou gameplay acceptance na každé deklarované podporované kombinaci; package smoke a permission revocation během inputu. Commit `feat: add capability-verified Wayland integration` nebo `docs: record verified Wayland automation limitations` podle skutečného výsledku.

**Gate M4:** plná povinná kontrola pro uvedené prostředí. GUI/capture-only zůstává užitečná dostupná funkce, ale není splněním gameplay gate.

## Verifikace a uzavření každé etapy

Příkazy z root workspace; na Windows nejprve `. ./tools/rust-env.ps1`, na ostatních hostech nastavit nativní dependencies podle `docs/rust/README.md`:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
metinbot --self-test --self-test-report <isolated-output>/self-test.json
```

Spouštět na nativním hostu každého deklarovaného targetu s jeho SDK/OpenCV. Windows-only integration tests se nekompilují na ostatních OS přes target cfg; společné testy se nikdy nevynechají kvůli OS. Platformní testy v test app jsou oddělené od gameplay acceptance. CI headless má zvlášť zdokumentované skipy GUI oprávnění; skip není PASS celé funkce.

Každý task uzavřít aktualizací parity/acceptance s commit ID, příkazy, výsledky a zbývajícími omezeními. Při nečekaném chování nejprve určit příčinu, nepřidávat fallback. Po dokončení relevantní testy rozšiřovat pouze kvůli změně, selhání nebo konkrétní neuzavřené obavě.

## Odhad a vstupy potřebné až pro provedení

Jde o 10 etap M1 a tři samostatně ověřované backendové etapy. Pracnost dominuje Windows runtime parity + editor + skutečná acceptance; počet řádků Rustu není měřítko dokončení. Kalendářní termín stanovit po T0/T1, kdy bude znám stav toolchainu, native OpenCV a úplný baseline. Tento plán neobsahuje nepodložený příslib počtu dní.

Pro M2/M3/M4 je potřebná interaktivní Linux/macOS session a spustitelný herní klient v daném prostředí. Bez nich lze doložit architekturu, build a test app, nikoli herní podporu. Pro signing jsou později potřebné signing credentials; nemají se zapisovat do repozitáře ani do plánu.

## Kontrola tohoto plánu

- P01–P31 mají vlastní task a gate; úplný source-test mapping je explicitní první výstup T0.
- Kontrakty core/platform/runtime/editor/storage mají jediného vlastníka a nejsou závislé na GUI nebo Windows číslech.
- Review Focus je pokryt konkrétními red testy; packaging a remote proof mají samostatné gates.
- M1, M2, M3 a M4 mají odlišná tvrzení o podpoře; neznámé OS schopnosti jsou explicitní blokery daného backendu.
- Python odstranění a ochrana existujících dat nejsou odloženy na neurčité „později“.
