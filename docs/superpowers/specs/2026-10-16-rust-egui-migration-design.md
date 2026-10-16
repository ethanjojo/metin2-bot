# MetinBot: návrh přepisu do Rustu a egui

Datum: 2026-10-16. Výchozí revize: `54ac124`.
Stav: návrh k revizi; implementace ani migrace uživatelských dat nezačala.

## 1. Cíl a hranice

Samostatná desktopová aplikace v Rustu s egui/eframe, která zachová všechny dnešní uživatelské funkce na Windows. Společné jádro, editor, profily a GUI budou přenositelné na Linux a macOS. Podpora hry na konkrétní platformě vzniká až ověřením capture, vstupů a bezpečnostních kontrol v reálném klientu, případně ve Wine/kompatibilní vrstvě.

M1 = úplná Windows parita a vydání náhrady Python aplikace. Součástí M1 je nativní build/test společných částí a spuštění GUI na Linuxu a macOS, platformní kontrakty a pravdivé zobrazení nedostupných funkcí. M2 = samostatně ověřený Linux X11 backend. M3 = samostatně ověřený macOS backend. M4 = Wayland backend v konkrétních ověřených prostředích. M2–M4 jsou součástí roadmapy, nejsou podmínkou Windows cutoveru. Samotné M1 nesmí být označeno jako multiplatformní herní podpora.

Neměníme herní algoritmus, nepřidáváme WASD průzkum, léčení, login, frontu Ctrl-drag, souběžné klienty, čtení paměti hry ani injekci do procesu. Nové GUI zachová operace a význam stavů, nemusí kopírovat Tk widgety pixel po pixelu. UI a dokumentace pro uživatele zůstanou anglicky; vlastní názvy se zachovají.

## 2. Zjištěný základ

README, `metin_bot/`, `tests/`, `packaging/`, `tools/` a `.github/workflows/release.yml` byly přečteny při plánování. Inventář obsahuje 256 definic `test_*`; jde o statický počet, nikoli výsledek spuštění testů. Pracovní strom byl před zápisem návrhu čistý. Staré návrhy v docs a historické výsledky nejsou důkazem dnešního runtime chování.

Současný Windows transport je důležitý externí behaviorální kontrakt: keyboard i mouse down/up používají zprávy adresované původnímu HWND/PID, pohyb kurzoru používá Windows input. Nahrazení této kombinace univerzálním globálním input wrapperem není součást přepisu. Windows capture dnes čte viditelnou klientskou oblast obrazovky přes Pillow, nikoli skryté nebo minimalizované okno. Zachovat tyto podmínky; Rust implementace použije GDI capture této stejné viditelné oblasti.

## 3. Matice parity

Každý řádek musí získat Rust test/scénář a odkaz na důkaz v acceptance dokumentu. Značka „hotovo“ nesmí vycházet pouze z počtu testů.

| ID | Zachované chování | Současný zdroj / testy | Etapa plánu |
|---|---|---|---|
| P01 | Výběr, refresh a vytvoření pojmenovaného profilu; poslední výběr odděleně | app, profiles; test_app, test_profiles | T2, T7 |
| P02 | Výběr konkrétního herního okna; nevybrat libovolné z více oken | windows; test_windows | T4, T7 |
| P03 | Snapshot zmrazí kalibrační obraz; live/frozen/stale a původ snímku | calibration, runtime, preview_ui; test_calibration, test_preview_ui, test_runtime | T3, T6, T7 |
| P04 | HSV barva včetně kruhové hue vzdálenosti, rozměry/plocha, region/masky a automatické vyloučení panelu | vision; test_vision | T3 |
| P05 | Stabilita kandidátů, pořadí podle vzdálenosti od středu a původní tie-break | vision; test_vision, test_controller | T3, T5 |
| P06 | Panel region + odlišitelná pevná šablona, grayscale matching a potvrzení/potvrzené zmizení | vision, config, controller; test_vision, test_config, test_controller | T2, T3, T5 |
| P07 | Jeden cíl, acquisition/attack timeout, settling, neúspěšné cíle a cooldown, rotace E s limitem | controller; test_controller, test_runtime | T5 |
| P08 | Start ukládá, požádá o aktivaci; 3 s delay, 0,5 s stabilní focus, 12 s timeout; F7 pouze v cílové hře | app, worker; test_worker, test_app | T4, T5, T7 |
| P09 | F8 má přednost před F7; Pause ruší i čekající start; žádné vstupy z preview | worker, runtime; test_worker, test_runtime | T4, T5 |
| P10 | Focus loss, překrytý click point, ruční pohyb kurzoru, rozměry, capture/input chyby zastaví běh | windows, runtime; test_windows, test_runtime | T4, T5 |
| P11 | Cleanup down/up v obráceném pořadí; původní identita cíle, nikdy reused handle; uzavření uvolní vstupy i hotkeys | windows, worker; test_windows, test_worker | T4, T5 |
| P12 | Editor world/panel/template/masks: vytvořit, posunout, resize handles, delete jen vybraný prvek; validace containment | calibration, editing; test_calibration, test_editing | T6 |
| P13 | Gesture až do mouse-up mění jen preview; Escape cancel; undo/redo 32 stavů včetně šablon, bez duplicit | editor_tools, editing; test_editor_tools, test_editing | T6 |
| P14 | Zoom až 800 %, bod pod kurzorem, pan prostředním tlačítkem, fit, clipping/padding, resize viewportu | editor_tools; test_editor_tools | T6 |
| P15 | Aktivní krok/handles, hover jiného kroku, dirty stav, sidebar scroll/hide, empty state; tab nepřeskakuje na první pole | editor_ui, app; test_app, test_sidebar_scroll, test_preview_ui | T6, T7 |
| P16 | Číselná pole: psaní, šipky, krok/mezní hodnoty z jedné validace; light/dark včetně Windows chrome | number_input, theme, window_chrome; test_number_input, test_theme | T2, T7 |
| P17 | Až 8 skillů; jméno, přesun celé konfigurace, smazání; prázdné draft řádky se přeskočí | buff_ui, config; test_buff_ui, test_config | T2, T7 |
| P18 | Záznam Ctrl/Shift/Alt + písmena/čísla/F1–F12, CZ layout, F7/F8 rezervace, Escape/focus cancel, delete jen volitelný bind | hotkeys, keybind_entry; test_keybind_entry, test_buffs | T2, T4, T7 |
| P19 | Refresh interval každého skillu 10–86400 s, s/min bez změny délky, vlastní wait, HH:MM:SS, reset jen jednoho timeru | buffs, buff_ui; test_skill_intervals, test_buff_scheduling, test_buff_ui | T5, T7 |
| P20 | Inactive toggle: samostatná icon oblast, aktivní/neaktivní PNG v JSON, nezávislé crop rozměry, reuse souřadnic nikoli obrázku | config, calibration, buff_ui; test_buff_ui, test_hotbar_buffs, test_calibration | T2, T3, T6, T7 |
| P21 | Toggle: 0,90 match, 0,08 náskok; active-only absence <=0,65; 3 čerstvé frame; unknown/černý frame není absence | buff_detection; test_buff_detection, test_hotbar_buffs | T3 |
| P22 | Skilly mezi cíli; dismount pouze při skutečném použití; wait/ack, remount podle fight_mounted; nejvýše jednou před dalším pokusem o cíl | buffs, controller; test_buffs, test_buff_scheduling, test_runtime | T5 |
| P23 | Mount On/Off uložen mimo profil, default Off; změna pauzuje a posílá deklaraci i bez Save; failed toggle => unknown do explicitního potvrzení | theme, switch_ui, worker; test_switch_ui, test_worker, test_runtime | T5, T7 |
| P24 | Save/refresh stejné identity zachová session/timery/mount; změna behaviorální buff config restartuje cyklus, přejmenování nikoli | worker, buffs; test_worker, test_skill_intervals | T5 |
| P25 | Active time bez pauz, targets ended nejsou „prokázané killy“, clicks/unconfirmed; reset statistik nemění timery; nová identita resetuje session | session, run_ui; test_session, test_run_ui, test_worker | T5, T7 |
| P26 | Přísná schema/readiness, neúplný draft lze uložit, start ne; UUID/PNG containment a atomické zápisy, staré JSON varianty | config, profiles; test_config, test_profiles | T2 |
| P27 | Kanonická user data, first-run import/cancel, explicitní import jen do prázdna, bez overwrite/merge, originály zachovány | data_paths, data_import_ui; test_data_paths, test_cli | T2, T7 |
| P28 | --version, --data-dir, --import-data, --log, --self-test + --self-test-report a omezení kombinací | cli, self_test; test_cli, test_build | T1, T2, T9 |
| P29 | Windows UAC/privilege kontrola, installer upgrade/uninstall se zachováním dat, bez Pythonu na cílovém PC | permissions, packaging, tools; test_launcher, test_build, verify_launcher.ps1 | T4, T9 |
| P30 | Build identity, tag/ref kontrola, prerelease/release, checksum publikovaného artefaktu, permissions CI | version, release_metadata, workflow; test_release, verify_publish.ps1 | T9 |
| P31 | Lokální logy bez automatického uploadu/screenshotů; capture/permissions diagnostika, izolované UI fixtures | diagnostics, tools/check_capture.py, tools/check_permissions.py, test_gitignore | T1, T4, T7, T9 |

Portovat také assertiony stávajících testů mimo tabulku; T0 vytvoří úplné mapování všech 256 testů. Tk/source-text/packaging testy nahradit testem stejného kontraktu, nikoli kontrolou Rust zdrojového textu. Chybu objevenou při portu evidovat jako samostatnou opravu s rozdílem proti baseline; nekonzervovat ji mlčky jako paritu.

## 4. Architektura a soubory

Root Cargo workspace se čtyřmi crates; Python dočasně zůstává referencí, nikdy knihovnou Rust runtime. Jeden executable `metinbot`.

```text
Cargo.toml, Cargo.lock, rust-toolchain.toml
crates/metin-core/src/{lib,profile,validation,geometry,shortcut,controller,buffs,session,time}.rs
crates/metin-platform/src/{lib,contract,frame,capabilities,error}.rs
crates/metin-platform/src/windows/{mod,capture,input,windows,permissions,hotkeys}.rs
crates/metin-platform/src/linux/{mod,x11,wayland}.rs
crates/metin-platform/src/macos/{mod,capture,input,permissions,windows,hotkeys}.rs
crates/metin-vision/src/{lib,detector,tracking,panel,buffs,templates}.rs
crates/metin-desktop/src/{main,lib,cli,bootstrap,diagnostics,self_test}.rs
crates/metin-desktop/src/runtime/{mod,worker,commands,snapshot,input_executor}.rs
crates/metin-desktop/src/storage/{mod,profiles,preferences,paths,import,migration}.rs
crates/metin-desktop/src/editor/{mod,calibration,gesture,viewport,history}.rs
crates/metin-desktop/src/ui/{mod,shell,profiles,preview,calibration,buffs,settings,overview,keybind,numeric,theme,permissions}.rs
crates/*/tests/                    # behavioral integration tests
tests/parity/{manifest.json,profiles,frames,traces}/
docs/rust/{parity,acceptance,platform-support,performance}.md
tools/{build-rust.ps1,verify-rust-installer.ps1,package-linux.sh,package-macos.sh}
.github/workflows/{rust-ci.yml,release.yml}
```

Závislosti: core bez OS/GUI/OpenCV; platform závisí na core a target-specific SDK; vision závisí na core, frame kontraktu platform a OpenCV; desktop skládá vše. V core nejsou HWND, VK čísla, Cocoa objekty, XID, DBus ani egui typy. `unsafe` pouze u zdokumentovaných FFI hranic. Žádná service síť, databáze, plugin systém nebo obecný event bus.

GUI na hlavním vlákně. Jediný worker vlastní běhový stav controller/buffs/session, připravené šablony a cílovou session. OS event loop/capture callback může mít systémem vyžadované pomocné vlákno; nesmí vlastnit druhý controller. Typed command fronta je omezená; při zaplnění UI oznámí odmítnutí. Pause/Stop/Shutdown používají samostatnou atomickou cancellation generation a wakeup, nikdy nečekají za command frontou. Následný Start nesmí „odmazat“ starší Stop. Jeden latest snapshot slot; frame se sdílí immutable, staré náhledy se zahazují. Překreslení egui jen na událost/nový frame/deadline viditelného timeru; žádný trvalý busy loop ani detekce ve render funkci.

## 5. Veřejné kontrakty pro implementaci

V `contract.rs` definovat `TargetToken` jako neprůhlednou identitu session (ne jen PID/title), `TargetInfo`, `TargetState`, `FrameSize`, `FramePoint`, `ScreenPoint`, `CaptureEpoch`, `FrameId`, `CapturedFrame`, `CapabilityReport`, `InputEvent`, `PlatformEvent`, `StopToken`, `Deadline` a `PlatformError`. Tokeny nelze serializovat do profilů. Windows token váže HWND+PID+životnost procesu; X11 XID+ověřenou identitu; macOS window+owner+session; Wayland portal session+stream identitu, nikoli vymyšlený globální handle.

```rust
pub trait PlatformBackend: Send {
    fn capabilities(&self) -> CapabilityReport;
    fn list_targets(&mut self) -> Result<Vec<TargetInfo>, PlatformError>;
    fn attach(&mut self, target: TargetToken) -> Result<(), PlatformError>;
    fn inspect(&mut self) -> Result<TargetState, PlatformError>;
    fn request_activation(&mut self) -> Result<bool, PlatformError>;
    fn capture(&mut self, deadline: Deadline, stop: &StopToken)
        -> Result<CapturedFrame, PlatformError>;
    fn send(&mut self, event: InputEvent) -> Result<(), PlatformError>;
    fn release_owned(&mut self) -> Result<(), PlatformError>;
    fn drain_events(&mut self) -> Result<Vec<PlatformEvent>, PlatformError>;
}
```

`TargetState` má ověřenou identitu, viditelnost, foreground, client pixel size, transform epoch, privilege/input permission a hit-test podporu. Neověřitelný údaj je `Unknown`, nikdy automaticky `true`. `CapabilityReport` popisuje capture, target identity, foreground, hit-test, input, ruční pohyb kurzoru, global start/stop a důvod nedostupnosti. Nedostupná povinná kontrola zablokuje automatizaci; editor a import zůstávají dostupné. `Unsupported` backend není náhradní input cesta.

`CapturedFrame`: immutable tightly packed BGR8 pixely, rozměry/stride, monotónní capture čas, FrameId, TargetToken a CaptureEpoch. Převod RGB/BGRA/Retina/PipeWire bufferů je jedině na platformní hranici. `FramePoint` jsou fyzické pixely klientské oblasti; `ScreenPoint` jsou souřadnice desktopu. egui points jsou samostatná jednotka. Jeden transform pro kreslení, hit-testing i input; změna DPI/velikosti/zdroje zneplatní epoch a pozorování. Žádné automatické přepočítání původní kalibrace na jiné rozměry.

Core exportuje `Controller::step(now: MonoTime, observation: &Observation) -> Option<Action>` a `Controller::ack(now: MonoTime, action_id: ActionId, result: ActionResult)`. Akce je tagged enum `Click`, `Rotate`, `Mount`, `Skill`; ne struktura s libovolnými nullable poli. Každá neukončená akce má jedinečné ID, právě jedno dokončení a session generation. Opožděný ack nesmí přepsat nový stav. Mount rozlišuje deklarovaný/odvozený `Mounted`, `OnFoot`, `Unknown`; input queued neznamená vizuální potvrzení.

Runtime exportuje `RuntimeHandle::send(command: Command) -> Result<(), CommandRejected>`, `stop()`, `shutdown()` a `latest() -> Arc<RuntimeSnapshot>`. Příkazy: Configure, Detach, Arm, Toggle, DeclareMount, ResetStatistics, ResetSkillTimer; každý target-specific příkaz nese `SessionKey { profile_id, target_token }` a request revision. Snapshot nese stejnou identitu, applied revision, frame epoch, enum stavu, důvod, mount, timery, statistiky a případný immutable preview. Draft editoru není druhý běhový profil; Save/Start odešle jeden validovaný immutable Profile, UI čeká na odpovídající ack.

Čas v core je monotónní injektovaný `MonoTime`/`Duration`. Timery měří aktivní čas; wall clock jen pro logy. Prodlevy input executor čekají přerušitelně po nejvýše 20 ms. Během capture se nedrží žádná klávesa/tlačítko; capture má deadline a revokace oprávnění nesmí zavěsit shutdown. Explicitní release je povinný na všech cestách; Drop je poslední pojistka, nikoli jediná cleanup strategie.

## 6. GUI a detekce

Zachovat tři pracovní části Calibration, Buffs, Settings, přehled běhu/statistik a sklápěcí sidebar. Zamrzlý snímek je vlastní kalibrační snapshot, živý frame ho nenahradí během editace. Vektory/handles se kreslí nad texturou, drag neprovádí plnou detekci. Undo obsahuje profil i immutable template assets. Timer UI jen zobrazuje worker snapshot, nikdy neplánuje skill.

Použít egui + eframe s wgpu, přibalený font s českými znaky, semantic light/dark palette, klávesovou navigaci a jasný focus. Na macOS systémové Command+Z/Y pro editor vedle zachování Ctrl variant; herní bindy nadále Ctrl/Shift/Alt, bez automatického překladu Ctrl na Command. Rezervace F7/F8 a skutečně přidělené systémové zkratky se zobrazují pravdivě. Windows nativní chrome se zachová platformním hookem, jiná OS používají jejich podporované dekorace.

OpenCV zůstává jediná detekční implementace přes Rust bindings. Vypnout nepoužívané moduly; potřebné image codecs a imgproc; žádná highgui okna. Přesné verze Rust toolchain, egui/eframe a OpenCV/bindings připnout až po T1 build probe na všech třech OS a zapsat do lock/build dokumentace; neodhadovat verze z data plánu. T1 je povinná decision gate pro tuto konkrétní kombinaci, nikoli příslib hotového build prostředí.

Diferenciální testy: přesné pořadí kandidátů/regiony/akce; match skóre tolerance max 1e-5 na stejném referenčním OpenCV buildu. Případy u prahů musí mít totožné rozhodnutí i když se float skóre liší v toleranci. Cross-OS odchylka mimo limit vyžaduje vyšetření a doloženou změnu kontraktu, ne plošné rozvolnění testu. Python `round` half-to-even a řazení/Unicode nesmí být nechtěně nahrazeno Rust `round` half-away-from-zero nebo ASCII lowercasing. Reprodukovat současný převod souřadnic a řazení profilů.

## 7. Data a migrace

Preferujeme zachovat současný JSON formát při M1, protože existující profily a uživatelská data jsou skutečný kontrakt. Zachovat všechna pole, UUID, template_file PNG, embedded Base64 PNG buffů, regiony, units v sekundách, selection.json i ui-settings.json. Nevkládat schema_version do starého formátu bez potřeby. Rozšířit validaci `process_name` na bezpečný neprázdný basename bez cest; je to pouze filtr/hint při výběru cíle, nikoli runtime identita. Stávající `.exe` hodnoty zůstanou beze změny, na jiném OS lze okno explicitně vybrat; Wine host process se nesmí automaticky zaměnit za vybranou hru.

Jedna normalizační importní hranice načte současné i dříve podporované varianty (`initial_mounted`, starý globální interval buffů, legacy config.json). Vrací pouze dnešní typed Profile. Zachovat původní známé defaults/konverze; běh nemá žádnou „legacy větev“. Pokud budoucí backend vyžádá nové persistentní údaje, nejprve samostatně specifikovat verzi a bezpečnou migraci; neschovávat je do process_name.

Windows: `%LOCALAPPDATA%/MetinBot`. Linux: `$XDG_DATA_HOME/MetinBot`, jinak `~/.local/share/MetinBot`. macOS: `~/Library/Application Support/MetinBot`. Stejná vnitřní struktura, settings a logs pod tímto kořenem jako dosud; `--data-dir` explicitně přepíše celé umístění a vypne auto-discovery. Detekci kořene držet v jednom modulu. Žádný zápis do instalačního adresáře.

Před cutoverem testovat jen kopie v explicitním testovacím kořeni. Nedovolit dva Rust writery (OS file lock), uživatel musí před předáním kanonického kořene zavřít Python verzi, která tento lock nezná. Import validuje kompletní strom, odmítá symlinky/junction escape, nebezpečné názvy, duplicitní UUID, neplatné a chybějící assety. Všechny legacy validace z config/profiles zachovat včetně NaN/type kontrol, max 30 masek, PNG buff 5–128 px a Base64 do 100000 znaků. Limity velkých externích PNG doplnit zkouškou hlavičky před alokací v mezích stávající maximální kalibrace 16384 px na osu; počet/dekódování omezit, nepovolit decompression bomb.

Import jen do prázdného cíle: staging vedle cíle, kompletní validace, atomické předání a uklizení staging při chybě; zdroj beze změny. Uložení profilu nejprve zapíše nový unikátní PNG, pak atomicky nahradí JSON; starý asset smaže až po úspěchu. Selhání nesmí poškodit předchozí profil. Před Windows cutoverem kopie celého user-data kořene; rollback starého buildu má použít snapshot před případnými zápisy z jiného OS. Žádné tiché slučování dvou uložišť.

## 8. Platformní realizace a limity

| Backend | Plánovaná cesta | Podmínka provozu |
|---|---|---|
| Windows x64 | `windows` crate: GDI visible client capture, Win32 enumerace/identity/focus/hit-test/hotkeys, dnešní window messages a guarded cursor move | Plná parita, UAC a klient ověřené v M1 |
| Linux X11 x64 | X11 client capture/geometry/identity/focus/hit-test, XTEST input, grab hotkeys; x11rb s konkrétními extensions | Ověřit Wine klienta a vlastnictví/release globálního inputu; globální transport není ekvivalent PostMessage |
| macOS arm64 (+ x64 po samostatném ověření) | ScreenCaptureKit, AX foreground/accessibility, CoreGraphics input, native hotkey event integration | Povolení Screen Recording/Accessibility a případně Input Monitoring; ověřit konkrétní kompatibilní klient |
| Linux Wayland | ScreenCast/PipeWire + RemoteDesktop portály, GlobalShortcuts; jedna explicitní vybraná portal session | Capture/input nezaručuje možnost ověřit target focus/obstruction/manual movement; bez potřebných důkazů automatizace zůstane nedostupná |

X11 a Wayland jsou explicitní backendy vybrané podle skutečné desktop session; žádné přepnutí na XWayland/globální input po selhání oprávnění. Wayland není obcházen spuštěním jako root nebo čtením /dev/input. Žádná platforma nesmí potichu vypustit kontroly P09–P11, aby prošla demo ukázka. Podporu zaznamenat po kombinacích OS/verze, display server/compositor, architektura, klient a kompatibilní vrstva/verze. Neexistuje tvrzení „funguje na všech Linuxech/Macích“.

Backend capability = technická možnost; acceptance = doložené chování konkrétního prostředí. GUI spuštěné bez permissions funguje jako editor, Start ukáže konkrétní důvod nedostupnosti. Oprávnění pouze přes dokumentované OS dialogy po explicitní uživatelské akci; žádné automatické elevace na Linuxu/macOS. Windows zachová současný manifest a privilege check.

## 9. Akceptace, distribuce a odstranění Pythonu

T0 uloží baseline verzi/commit/dependencies a skutečný výsledek Python suite; nic z historických dokumentů nepřebírá jako nový PASS. T1 změří capture/detection/end-to-end latency, CPU, peak RAM, idle GUI CPU, start a velikost instalace na stejné sestavě. Windows release nesmí mít nevysvětlené >10% zhoršení p95 runtime latency nebo peak RAM proti baseline v opakovaném stejném scénáři; GUI idle bez trvalého redraw. Výkonnostní odchylku řešit před release nebo výslovně schválit jako změnu cíle.

Povinné `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo build --workspace --release --locked` na nativních CI hostech pro každý deklarovaný build target. GUI/capture/permissions/herní acceptance se provádí v interaktivní session s důkazy, ne z headless PASS. Sanitizované obrázky a replay traces bez lokálních cest a osobních dat; žádné automatické screenshot uploady.

Windows zůstává Inno Setup, bez PyInstalleru v Rust balíku. Linux archive + desktop entry jako první distribuční formát, macOS .app bundle + dmg. Každý nativní balík obsahuje potřebné OpenCV knihovny a licence, self-test bez Pythonu, build identity a SHA256. Windows upgrade odstraní starý `_internal`, uninstall zachová data. macOS signing/notarization vyžaduje reálné credentials; bez nich je výsledek výslovně unsigned test build. Ověřit remote tag -> commit -> workflow -> stažený asset -> checksum. Publikace vyžaduje samostatný pokyn k vydání, není součástí této přípravy plánu.

Python zdroje během M1 pouze referenční a aktivně opravované jen při samostatném schválení; neudržovat dvě produktové varianty. Po všech P01–P31 a Windows acceptance přepnout default launcher/release na Rust, odstranit metin_bot/, Tk/Python deps, PyInstaller packaging a nahrazené nástroje/testy. Přenosné fixture, rozhodovací evidence, import existujících dat a historie Git zůstanou. Python reference/generátor testovacích očekávání se při cutoveru také odstraní; golden fixtures pak runtime Python nepotřebují.

## 10. Primární zdroje ověřené při plánování

- [egui/eframe](https://github.com/emilk/egui), [eframe API](https://docs.rs/eframe/latest/eframe/).
- [Rust OpenCV bindings a build requirements](https://github.com/twistedfall/opencv-rust).
- [Microsoft Rust for Windows](https://learn.microsoft.com/en-us/windows/dev-environment/rust/rust-for-windows).
- [XTEST](https://www.x.org/releases/X11R7.5/doc/Xext/xtestlib.html).
- [Apple ScreenCaptureKit sample](https://developer.apple.com/documentation/screencapturekit/capturing-screen-content-in-macos?language=objc), [AX trust](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions).
- [ScreenCast portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html), [RemoteDesktop portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html), [GlobalShortcuts portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html).

API dokumentace podporuje návrh použitých cest; neprokazuje kompatibilitu s Metin2 klientem. Přesné runtime/SDK verze a dostupnost systémových metod se potvrdí v T1 a příslušné platformní etapě.
