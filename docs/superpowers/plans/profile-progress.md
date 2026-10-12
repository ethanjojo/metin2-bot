# SDD ledger — plan: docs/superpowers/plans/2026-10-11-profile-target.md

Spec and plan explicitly approved; native execution in this chat.
Ruling: workspace is not a Git repository. Edit approved workspace directly, no Git scripts, commits or worktrees; durable ledger is here.
Pre-flight: task1 Profile -> task2/3/4/5 matched. task2 Candidate -> task3 Observation and task4 Sample matched. task3 Action -> task4 InputDriver.click matched. task4 Runtime -> task5 Worker matched.
Ruling: migration changes central types across all layers. During the sequential migration old consumer tests temporarily import removed types; do not claim whole-suite completion until all consumers are updated. No legacy runtime fallback will be kept.
Task1 RED: new Profile import absent, confirmed before implementation.
Task1 component tests 8/8 PASS. Task2 RED: missing new detector interfaces. Shared validate_template will serve persistence and vision.
Task2 component tests9/9 PASS; task3 controller9/9 PASS. Ruling: rotation settle needs regular observations; sparse unit-test times corrected to observe completion of settling, not change production deadlines. Task4 click RED missing method before implementation.

Task4: Windows16/runtime9 PASS; guarded click retains cancellation and cursor acknowledgement. Worker moved to worker.py to isolate thread lifecycle from Tk editor; app imports the single Worker implementation.
Task5: calibration6 PASS, including small template RED -> Region minimum0.1% GREEN; actual template minimum5px enforced separately. UI migrated; controller flickering-panel deadline RED -> GREEN. Whole suite70/70 PASS before added detach case.
Task5: detach-runtime-before-F7 test RED missing Worker.clear; implemented explicit clear on window discovery to remove stale binding.

Task5 complete: profiles/click/panel migration integrated; superseded queue docs marked, old runtime/example removed. Template position added for interactive edit persistence.
Interactive editor: user explicitly requested simpler UI and direct move/resize. Scoped implementation: calibration tabs, active/completed steps, eight handles, move/resize/clamp, individual masks, Escape, uncommitted preview, persisted template_region. Geometry tests RED missing editing module -> GREEN7; canvas regressions verify existing region movement and cancellation.
Final fresh review (requesting-code-review / executing-plans skill): three Important findings, zero Minor. One fix pass, no re-review.
Final: fixed new-mask None index crash — test_new_mask_draws_preview_and_commits_without_index RED TypeError -> GREEN, full suite89/89.
Final: fixed seeking click/rotate on unconfirmed panel absence — test_seeking_requires_confirmed_absence_after_panel_flicker and test_seeking_does_not_rotate_on_transient_absence RED action -> GREEN, full suite89/89.
Final: fixed F7 restart overriding F8 — test_f8_dominates_simultaneous_f7_event RED mouse-down -> GREEN, full suite89/89.
Additional drag edge: collapse new rectangle at start must discard previous preview — test_collapsing_new_rectangle_does_not_commit_previous_preview RED changed profile -> GREEN.
Native UI check: existing area moved with1021x571size preserved and resized957x522. Fixture separate profiles under diagnostics, no game inputs. Live game attack remains unverified (admin mismatch). Compileall PASS, pip check clean.
Native confirmation: new mask creation PASS; UI Save -> ProfileStore reload2masks/template_region/PNG, ready=True. Final screenshot diagnostics/editor-final.png. No deferred minors. Workspace retained because no Git snapshot exists.

User accepted all four editor additions: calibration undo/redo, cursor zoom/pan, input-free calibration test, unsaved-change protection. Implemented one History and Viewport, preview_only in Runtime/Worker, guarded profile/new/close transitions, and corresponding UI controls/docs.
RED: missing editor_tools module -> geometry/history tests GREEN; missing preview_only keyword -> runtime/worker tests GREEN. App transition tests cover cancellation, failed save, saved-state dirty reset and zoomed picker.
Fresh reviewer (requesting-code-review skill): two Important findings, no Critical. Selector after saving during switch RED -> load_profile selects canonical ID GREEN. Test exit disconnected runtime RED -> restores paused configuration GREEN. Failed diagnostic capture stale display RED -> explicit stale label GREEN. No re-review requested.
Final 108/108 unittest PASS, compileall PASS, pip check clean. Native isolated fixture verified zoom, drag at zoom, Ctrl+Z/Y, reset, diagnostic display/start disabled, exit and unsaved-close cancellation. Middle-button pan verified by unit tests only, because native tool has no middle-drag API. Screenshot diagnostics/editor-tools-final.png; fixture profiles separate from real Verzum. Live game attack remains unverified.

User requested understandable run state, counters and timer. Implemented compact overview above editor, phase deadline, active time, confirmed panel completions, acknowledged clicks, acquisition/attack failures and explicit reset. Session owned by Worker, immutable snapshot to UI, preserved across same-profile save/pause/test; profile/window change resets.
Fresh review: fixed active interval lost on cancellation, delivered but interrupted click not counted, stale failed sample bypassing identity filter. Regression coverage added; worker integration checks retention/reset/window transition, UI snapshot formatter checks nonzero counters and deadline. No second review loop.
Final 115/115 unittest PASS, compileall PASS, pip check clean. Native isolated overview/test/reset inspected; screenshot diagnostics/session-overview.png. Actual game input and live statistics remain unverified. Updated README and verification together.

Click investigation: minimum candidate dimensions, configurable hover/hold, one measured relative SendInput path and button-only down/up. Review acceleration overshoot and per-event focus/F8 guards fixed with regression coverage. User confirmed minimum dimensions remove small false positives.
User run 16:15:25 accepted SendInput down but aborted on async button-state timeout. Removed invalid delivery requirement after checking Microsoft GetAsyncKeyState access limitations; panel remains authoritative selection acknowledgment. Regression RED async state false -> SafetyError; GREEN full hold/release and click count. 120/120 tests PASS, compileall PASS, pip check clean. Docs/example updated; actual client response after final change remains unverified and requires restart. Do not report original click failure resolved from unit tests.

User confirmed physical click selects target, shows panel and attacks while elevated bot's accepted SendInput clicks do not. Classic client source explicitly dispatches WM_LBUTTONDOWN/UP with client coordinates. Mouse buttons now use one addressed PostMessageW path; original SendInput down/up removed. Movement/keyboard remain SendInput. Driver retains original window/clientpoint for release even after F8/focus loss; panel alone confirms actual selection. Tests first RED unsupported new API -> GREEN across Windows/runtime/worker.
Fresh review found cleanup blocked forever after original HWND destroyed/reused. Added typed WindowGoneError; release retires ownership without posting to another process. Other errors remain visible. Final125/125 tests PASS, compileall PASS, pip check clean. Exact Verzum handling is still unverified; restart and limited live test required. Public classic client is supporting evidence, not proof of this server binary.

User confirmed addressed clicks now attack and destroy successive metins. Approved all seven compact-UI proposals in chat; bounded refinement, direct execution. Implemented compact header and session strip, single guarded Start/Pause, authored monochrome icons/tooltips, zoom controls, sidebar collapse, contextual calibration groups, detection overlay visibility and freshness labels. Game input/controller unchanged.
Native bounded inspection caught default ttk icon-button width; explicit content width corrected. Fresh review caught retained running/armed status after window removal and hidden frozen-frame worker errors. Both fixed with regression coverage. Final131/131 tests PASS, compileall PASS, pip check clean. Isolated native fixture verified mode/masks, collapse/reopen, zoom125%, overlay hide; preview height~570px vs previous~330px. Real profile untouched, fixture input blocked. Minimum-size drag did not resize: no claim of native minimum-size verification. User restart needed for new UI.

Stable-message fix: reproduced 25px canvas jump in Tk. Shared fixed-height single-line messages with full-text tooltip and info dialog; snapshot loader in reserved toolbar space, one pending request, explicit cancellation on context changes. 134/134 tests PASS, compileall PASS, pip check clean. Full isolated App geometry unchanged across messages/loader/timeout; no game input or real-profile edits. Restart required.

User requested removal of redundant calibration-test button and mode. Removed App test state/transitions, Worker/Runtime preview_only API and mode-only tests; retained one stopped preview/start runtime path, detection overlay and panel score. README updated. 130/130 tests PASS, compileall PASS, pip check clean. Isolated App confirms controls and detection rendering, no game input. Earlier test-mode plan entries superseded. Restart needed.

Spinner placement corrected: replaced Snapshot icon with animated 16px frames inside LoadingButton. Removed external progressbar and slot, animation cancelled on completion/error/context switch/destruction. 131/131 tests PASS, compileall PASS, pip check clean; full isolated App confirms unchanged button/canvas geometry and restored icon/state. Restart needed.

Sidebar scroll fix: reproduced full-thumb short-content negative offset and stale scrollregion after off-screen shrink. Viewport-sized minimum region, top reset and disabled full scrollbar; shared wheel handling including canvas, deferred requested-size sync from descendant Configure. 134/134 tests PASS, compileall PASS, pip check clean. Both tabs verified in isolated App; no game input/profile edits. Restart required.

Auto-buff implementation: approved interval/target-end flow plus mount preference and Ctrl+G. Typed per-profile config, single BuffCycle in worker keyed UUID/HWND/PID, controller BUFFING steps acknowledged after release, foot/mounted preferences, preserved paused countdown and mount across Save/Refresh/disable, explicit mount override and unknown latch on failed toggle. New Buffy tab and README/profile example/design updated. Reviewer P2 countdown/state resets fixed and rechecked. Final150/150 tests PASS, compileall PASS, pip check clean. Full isolated Tk parsing/layout exercised; native activation failed and capture showed game, so no visual UI claim or live key-delivery claim. Test fixture stopped, real profiles untouched, user restart needed.

User requested key recording instead of typing. Shared readonly KeybindEntry for mount/all8 skills, canonical value separated from display hint, pause before capture, supported chords, reserved F7/F8, cancel Esc/focus/Tab, clear optional skill, consume editor shortcuts. 155/155 tests PASS, compileall PASS, pip check clean. Full isolated Tk App verifies nine recording controls and typed config after Alt+G/F4/Delete; no game input/profile changes. Restart needed.

## 2026-10-11 — Srozumitelný stav buffů a potvrzení mountu

- Samostatná skupina Stav mountu s explicitním potvrzením a lokální zprávou o čekajícím uložení. Draft zapnutí/vypnutí se zobrazuje ihned, staré worker samples nepřepíšou neuložené buffy. Kanonický runtime mount a interval zachovány.
- 157 unittest PASS, compileall a pip check PASS; celá izolovaná Tk App ověřila reakci textů a příslušnost tlačítka. Herní vstupy ani skutečné profily se neměnily.

## 2026-10-11 — Okamžité potvrzení a časování Ctrl+G

- Potvrzení mountu přímo workeru, bez Save a změny konfigurace, acknowledgement ID v Sample, ochrana identity UUID+HWND+PID. Odstraněna stará potvrzovací cesta přes configure. Prostá sekce bez boxu.
- Modifikátory před hlavní klávesou drženy 100 ms, po jejím uvolnění 50 ms; logging kombinace. Fyzické Ctrl+G podle uživatele funguje. Live reakce klienta na nové časování zůstává neověřená.
- 159 unittest PASS, compileall a pip check PASS; izolovaná celá Tk App přijala okamžité potvrzení bez Save a změn profilu. Žádné skutečné herní vstupy.

## 2026-10-11 — Ctrl+G: formát a přijetí kláves

- Časování nevyřešilo problém podle restartovaného klienta/logu. Ověřeny správné scancodes a elevated bot+klient. Keyboard path používá explicitní VK a levé modifikátory, před pokračováním kontroluje jejich held stav. Neúspěch pozastaví runtime před skilly, cleanup a unknown mount.
- 163 unittest PASS, compileall a pip check PASS. Živá reakce nové verze neověřená: uživatel restart odložil.

## 2026-10-11 — Window-addressed klávesy

- Nový log potvrzuje nepřijatý globální Ctrl. Keyboard přes PostMessage do původního okna, stejný jeden transport pro buffy i rotaci; správné Ctrl/G a Alt zprávy, kontrola HWND/PID, konečné uvolnění po focus loss a ochrana reused HWND. Stará SendInput keyboard cesta a _confirm_keys odstraněny.
- 166 unittest PASS, compileall a pip check PASS. Nový backend bez živého ověření: uživatel restart odložil. Přijetí zprávy není potvrzení sesednutí, žádný automatický fallback.

## 2026-10-11 — Intervaly až 24 hodin

- Kanonický limit intervalu 10–86400 sekund, UI tooltip a README. Uživatelův 240min interval (14400 s) zachován JSONem a pause/resume. 167 unittest PASS, compileall PASS. Toggle skilly nejsou implementovány; doporučená další změna vyžaduje aktivní ikonovou detekci buffů, nikoli jen lokální bool.


## 2026-10-11 — Režimy skillů a aktivní ikony

Každý BuffSkill má mode refresh/inactive a volitelnou BuffIcon (validovaný PNG vložený do JSON profilu). BuffConfig.icon_region je společná ručně vybraná oblast aktivních ikon, oddělená od hotbaru. Starší uložené skilly zůstávají refresh. Neúplná kalibrace je přípustná jako draft, ale blokuje běh. Picker používá existující náhled a obdélníkovou interakci, nezasahuje do oblasti metinů; miniatura zobrazuje uložený výřez.

BuffDetector vrací ACTIVE/ABSENT/UNKNOWN: korelace >=0.90 znamená aktivní, <=0.65 vyžaduje tři čerstvé snímky pro nepřítomnost. Mezilehlý výsledek, černý snímek nebo nesprávná geometrie znamená nejistotu. Freshness respektuje nastavený sample_interval. BuffCycle kontroluje poslední pozorování před každým skillem; inactive odešle pouze ABSENT, refresh vždy. Přeskočené skilly mají viditelný důvod. Keyboard transport zůstává stávající fungující window-addressed cesta.


## 2026-10-11 — Buffy při startu, dynamické řádky a samostatná oblast

- Úvodní BuffCycle je splatný při prvním startu; controller čeká na bezpečně nepřítomný panel a alespoň tři čerstvé snímky. Aktivní cíl nejprve dokončí. Interval začíná po plném dokončení sekvence; pause/resume zachovává zbývající čas. Aktivní a nejisté přepínací skilly zůstávají přeskočené.
- BuffList nahradil paralelní pole a osm předem vykreslených slotů jednou kolekcí celých SkillRow draftů. Přidání, odstranění, přesuny ↑/↓, zachování všech parametrů i obrázku, načtení pouze uložených řádků. Maximum osm zůstává kanonickou validací profilu. Nové widgety se zapojují do existujícího scrollování.
- Dedikované Mode.BUFF_AREA a BUFF_ICON: nezávislý výběr v celém snímku, vlastní popisek a úchyty, posun/resize/new/delete oblasti buffů. Výřez ikony musí být v oblasti buffů. Původní WORLD sdílení odstraněno.
- Nezávislá kontrola našla staré buffové PNG po změně rozlišení. Opraveno zrušením regionu a ikon v Calibration i UI, při zachování pořadí, zkratek, režimů a prodlev. Neúplná detekce blokuje běh.
- 185/185 unittest PASS (19.898 s), compileall PASS, pip check clean. Tk fixture PASS: oblast mimo WORLD, úprava oblasti, zachovaný svět, odmítnutí ikony mimo oblast, miniatura, přesun/odstranění a uložení. Herní vstupy zakázané ve fixture; skutečné profily nezměněny. Živý start buffů v klientu v této změně neověřen.


## 2026-10-11 — Aktivní a neaktivní vzhled hotbaru

- Uložená aktivní šablona uživatele měla proti přiloženým výřezům korelaci 0.67657 (neaktivní) a 0.71460 (aktivní). Oba výsledky byly UNKNOWN; log 22:33:55/58 potvrzuje odeslání pouze 3 a 4 po sesednutí. Převzetí raw nízkého prahu by nebylo bezpečné.
- BuffSkill.inactive_icon přidává volitelný neaktivní PNG stejné velikosti. Dvojice vyžaduje vítěze >=0.90 a odstup >=0.08; neaktivita má tři čerstvé snímky. Chybějící/nerozlišitelné podoby jsou UNKNOWN, bez fallbacku na prostou nepřítomnost. Detekce pouze mizející aktivní ikony zůstává explicitním nastavením bez inactive_icon pro skutečnou buffovou lištu.
- Přepínací řádek zobrazuje Aktivní ikona / Neaktivní ikona a obě miniatury. Neaktivní výběr drží velikost aktivního a ukotví se klikem na levý horní roh. Aktivní recapture ruší neaktivní; × dovoluje neaktivní vzor odstranit. Stejné vzory UI odmítá s výzvou obnovit snímek. Pořadí, uložení a změna rozlišení zacházejí s oběma vzory společně.
- Nezávislá kontrola našla resize mezi kontrolou a zmrazením snímku; kontrola regionu a aktivní ikony přesunuta až po zmrazení. Regrese pokrývá oba picker stavy. Při nové velikosti se picker nezapne s vymazanou kalibrací.
- 193/193 unittest PASS (23.125 s), compileall PASS, pip check clean. Dva lokální 35x35 hotbarové výřezy z uživatelových snímků v tests/data; aktivní/neaktivní rozlišení, absence obou, identická dvojice, serializace, rozměry, startup sekvence, miniatury a řazení ověřeny. Neaktivní startup odesílá Ctrl+G/2/3/Ctrl+G, aktivní Ctrl+G/3/Ctrl+G v controller testu.
- Tk fixture PASS: výběr obou stavů, automatická 35x35 velikost neaktivního vzoru, dvě miniatury, zachovaný world region, uložení, reorder a klasifikace. Fixture zakazuje herní vstupy, skutečný profil se nezměnil. Živá reakce klienta na tuto detekci nebyla ověřena; uživatel musí po restartu znovu zachytit oba vzhledy z aktuálních snímků.


## 2026-10-11 — Přepínací buffy mezi metiny, běžné podle intervalu

- Interval je nově pouze pro mode refresh. Inactive skilly se vybírají při potvrzeném ABSENT na bezpečné hranici před dalším cílem, bez čekání na dlouhý deadline. ACTIVE/ACQUIRING buffy nepřerušují. Samostatná aktivace přepínacího skillu zachovává deadline běžných buffů. Toggle-only nemá interval ani odpočet; aktivní/nejisté ikony samy nevyvolají sesednutí.
- Jeden _skill_indices sdílí výběr pro needed/begin a respektuje konfigurované pořadí. Před stiskem se znovu ověří aktuální stav. Interval se nastaví pouze při dokončení cyklu, který zahrnoval splatné běžné buffy. Pause/resume zachovává běžný deadline.
- Nezávislá kontrola odhalila globální blokování všech togglů po běžném buffu. Nahrazeno attempted_toggles po jednotlivých kanonických indexech a zapisuje se až při vrácení klávesové akce konkrétního togglu. Skilly, které vyprší během běžné animace nebo jiného togglu, se mohou před dalším cílem ještě použít; již použité se před stejným cílem neopakují. Kliknutí na cíl nebo start obnoví tuto evidenci.
- 199/199 unittest PASS (24.214 s), compileall PASS, pip check clean. Regrese: obnova togglu mezi dlouhými intervaly, zachovaný běžný deadline, žádný odpočet/mount při active/unknown toggle-only, čekání na target end, vypršení během běžné animace, dva samostatné toggly bez opakování prvního a UI popisky. Tk fixture PASS pro toggle-only/mixed stav bez ukládání skutečného profilu nebo herního vstupu. Živý klient touto změnou netestován.


## 2026-10-11 — Interval a odpočet každého běžného skillu

- Globální interval odstraněn z BuffConfig a UI. BuffSkill.refresh má interval 10–86400 s (default120), inactive nemá interval. Starý globální interval migruje při načtení do všech běžných skillů; vlastní hodnoty zůstanou zachované. Skutečný profil úspěšně načten v paměti bez zápisu.
- BuffCycle plánuje vlastní deadline po úspěšném uvolnění klávesy + vlastní prodlevě. Pause/resume zachová každý zbývající čas. Splatné skilly se provádějí v pořadí při společném sesednutí; Sample předává vlastní odpočty do řádků. Přesun zachová interval.
- Review našlo nekonečné opakování při intervalech kratších než další animace. Jediná evidence attempted_skills nyní brání opakování obou režimů před dalším pokusem o cíl. Regrese RED→GREEN, druhá kontrola bez dalších důležitých nálezů.
- 204/204 unittest PASS (21.196 s), compileall PASS, pip check clean. Skutečný Tk fixture PASS: vlastní intervaly/odpočty, přesun, skrytí intervalu u přepínacího režimu; jeden uživatelský profil ověřen bez zápisu. Herní vstupy zakázané ve fixture; živý klient netestován. README, specifikace a příklad profilu aktualizovány.


## 2026-10-11 — Volba sekund/minut u intervalu

- Běžný skill nabízí kompaktní volbu s/min; přepnutí přepočítá číslo beze změny délky. Celé násobky minuty se při načtení automaticky zobrazují v minutách (14400 s → 240 min). Jednotka je pouze prezentační stav řádku, profil a runtime zachovávají interval_seconds bez nové paralelní konfigurace.
- 205/205 unittest PASS (22.019 s), compileall PASS. Testy převodu, desetinných minut, přesunu a validace; skutečný Tk fixture potvrzuje událost comboboxu s→min→s, kanonické sekundy a načtení profilu bez zápisu. Herní vstupy neodesílány. README a specifikace aktualizovány.


## 2026-10-11 — Obousměrný výběr buffových vzorů

- Aktivní i neaktivní vzor mají vlastní volné tažení, miniaturu, × a Stejná oblast jako druhý vzor. Libovolný může být první. Nahrazení ani smazání jednoho nedotýká druhého; totožné PNG odmítány symetricky před mutací.
- BuffIcon ukládá volitelnou Region výřezu v JSON; jednotný capture_buff_icon získává nový PNG z aktuálního kalibračního snímku a kontroluje oblast buffů. Starší PNG bez souřadnic zachovávají detekci, opakování oblasti potřebuje nový výběr. Resize stále ruší oba vzory. Detector používá vlastní velikosti vzorů přímo; nepotřebné omezení shodných rozměrů odstraněno a ověřeno skutečnými hotbar snímky.
- 208/208 unittest PASS (30.829 s), compileall PASS. Regrese: region roundtrip, opačné pořadí, nezávislé mazání, containment a chybějící souřadnice bez mutace. Skutečný Tk fixture PASS: tažení obou vzorů, převzetí oblasti oběma směry, zachování fotky druhého vzoru, tlačítka mazání a JSON roundtrip. Nezávislá kontrola bez důležitých nálezů (47 cílených testů). Herní vstupy neodesílány a uživatelský profil nepřepsán. README a specifikace aktualizovány.


## 2026-10-11 — Oddělené statistiky/stavy a reset skillu

- Pevný přehled odděluje stav běhu, popsaný deadline a čtyři metriky relace. Hodnoty a popisky mají vlastní bloky; zpráva zůstává samostatnou stálou řádkou. Buffy mají tři oddělené hodnoty Automatiky/Nastavení/Cyklus; skilly vlastní HH:MM:SS, stav a reset. Přepínací skilly zobrazují kanonickou Presence ze Sample, nemají čas ani reset.
- Uživatel zvolil reset na nulu s použitím po dokončení cíle. Jediný Worker přijímá index s UUID/HWND/PID a očekávanou BuffConfig; odmítá zastaralé požadavky. Reset zachovává ostatní termíny, pauzu, mount i statistiky a neposílá vstupy. Controller provede skill po bezpečném ukončení cíle. UI vyžaduje uložené zapnuté buffy a aktuální připojení. Reset statistik zachovává deadline kroku i buff snapshot.
- 214/214 unittest PASS (27.171 s), compileall PASS. Regrese resetu během ACTIVE, pouze vybraného timeru, zachování pauzy, worker identity/config rejection, řazení a oddělení čítačů. Nezávislý reviewer bez důležitých nálezů (21 cílených testů).
- Skutečné screenshoty izolovaného Tk fixture ověřeny pomocí computer-use při1280×840 a980×640; přehled a odpočty bez překryvů, test stability geometrie zpráv prošel. verify_skill_intervals PASS, jeden uživatelský profil pouze načten. Fixtures ukončeny, skutečný bot/profil nepřepsán ani živé herní vstupy neodeslány. README a specifikace aktualizovány.


## 2026-10-11 — Mount jen v relaci, automatické předání přepínače

- Potvrzovací tlačítko a mount_choice_dirty odstraněny. Změna checkboxu okamžitě posílá bool přes existující identity-bound worker command; revision ack chrání poslední volbu před starým vzorkem. UI má Na mountu/Pěšky/Stav neznámý a krátké Nastavuji stav. Worker pozastaví běh; uživatel pokračuje sám.
- BuffConfig.initial_mounted odstraněno. Staré JSON pole se při načtení zahodí, příklad a serializer ho již nemají. BuffCycle začíná mounted=None. Stejná identita zachovává runtime mount přes Save/Refresh/Pause; nové HWND/PID má opět neznámý mount. Testy deklarují mount přímo jako stav relace, nikoli přes profil.
- 215/215 unittest PASS (26.347 s), následná rozšířená worker identity regrese9/9 PASS (12.499 s), compileall PASS. Nezávislý reviewer bez důležitých nálezů (57 cílených testů).
- verify_mount_switch PASS se skutečnými Tk widgety: okamžité odeslání oběma směry, čekání na revision, ochrana před starým ack, žádná mutace profilu a absence tlačítka. Screenshot compact fixture980×640 přes computer-use potvrzuje přehlednou mount sekci bez tlačítka. Živé herní vstupy neodesílány, uživatelský profil nezměněn.

Navržené další UI: sbalitelné pokročilé mount parametry, pojmenované skillové bloky, srozumitelné stavové štítky a jasný výběr kalibrační oblasti. Dark mode řešit jednotnou paletou všech ttk stylů, canvas okolí, ikon a stavů; preference tématu patří k aplikaci mimo serverové profily. Návrhy nejsou implementovány.


## 2026-10-12 — Kompletní vzhled, dark mode a kalibrační přehled

- Schválených šest úprav implementováno: Disclosure pokročilého mountu; volitelné pojmenování skillů; ohraničené bloky a kompaktní ovládání; sémantické textové štítky; miniatury kalibrace/hover zvýraznění; světlé/tmavé téma.
- theme.py má jednu paletu a validované UISettings v ui-settings.json mimo profily. Styly pokrývají ttk, nativní canvas/listbox, readonly/capture, popup combobox, spinner, tooltip a ikony včetně změny Spustit/Pozastavit. Téma nemění původní pixely ani význam detekčních barev. Staré statické definice hlavních stylů odstraněny.
- BuffSkill.name zachován při přesunu/mazání/JSON; execution_config vynechává pouze zobrazovací názvy pro porovnání plánu. Worker přejmenování zachová mount i zbývající časy a aktualizuje runtime popisky. Miniatury jsou z originálního snímku, cache obsahuje i vybranou masku; hover nemění editovaný Mode. Chybějící kalibrace přepínacího skillu má explicitní štítek.
- 219/219 unittest PASS (28.001 s), compileall PASS, pip check clean. Theme/preference/disclosure/name tests a rozšířená worker preservation regrese. Nezávislý reviewer bez důležitých nálezů (21 cílených testů). verify_theme PASS: preference mimo profil, zachované původní pixely a profil, žádné nové worker commands, miniatury a hodnoty disclosure.
- Native computer-use screenshoty ověřily tmavé980×640 a světlé1280×840, včetně skillových bloků, statusu, odpočtu, ikon a scrollbarů; bez překryvů. Herní vstupy ve fixture zakázané a skutečné profily netknuté. Testovací okna automaticky ukončena; otevřený skutečný bot potřebuje restart. README a specifikace aktualizovány.


## 2026-10-12 — Téma jako ikonové tlačítko

- Combobox a jeho StringVar/mapování odstraněny. Jediný toggle_theme přepíná light/dark, uloží preferenci a teprve po úspěchu aplikuje téma. Chyba zápisu ponechá původní téma a ikonu.
- Měsíc znamená přepnout na tmavé, slunce na světlé; dynamická nápověda akci pojmenuje. Ikony jsou součástí stávajícího generátoru a theme registry.
- 8 cílených unittest PASS, compileall PASS, verify_theme skutečných Tk widgetů PASS včetně invoke tlačítka oběma směry, uložené preference, nové ikony a zachovaných profilů/pixelů. README a diagnostické fixtures aktualizovány.


## 2026-10-12 — Prázdný náhled s ikonou a pokynem

- preview_ui renderuje centrovanou ikonu snímku48px, nadpis a krátký pokyn podle vybraného okna/pořizování snímku. Centruje se při resize, využívá stejnou paletu a icon renderer. Prázdný náhled neukazuje staré výsledky detekce.
- Refresh, chyba aktivace, timeout a přechod na živý náhled překreslí skutečný aktuální prázdný/loading stav. Původní text vlevo nahoře odstraněn.
- 221/221 unittest PASS (24.690 s), compileall PASS; testy centrální geometrie, kontextu a tématu, snapshot lifecycle a verify_theme rozšířené o skutečný prázdný canvas a vymazaný detekční footer. Native screenshot980×640 ověřil centrovaný layout v dark mode bez překryvů. Živé herní vstupy neodesílány.
- První kompletní běh skončil Tcl_AsyncDelete: disposable Tcl interpretery z UI testů mohly být finalizovány při GC workeru. WorkerTests nyní dokončí jejich callback cykly gc.collect na hlavním vlákně před startem vláken. Produkční shutdown již čeká na worker před root.destroy, nebyl měněn. Finální kompletní běh úspěšný.


## 2026-10-12 — Přepínače a odstranění duplicitního souhrnu buffů

- Všechny čtyři checkboxy mají jednotný Switch.TCheckbutton: automatické buffování, boj na mountu, aktuální mount a detekce. Nativní ttk proměnné, callbacky, Tab/mezerník a disabled zůstávají zachovány. Paletové image indikátory jsou cached na vlastním Tcl rootu pro light/dark, on/off/unknown a disabled. Mount deklarace neznámého stavu explicitně používá alternate; bez vybraného okna je přepínač nepřístupný.
- Souhrn Automatiky/Nastavení/Cyklus odstraněn včetně StringVars a update větví. Uložení ukazuje jen footer; runtime stav horní přehled; vlastní detekce/odpočty zůstávají u skillů.
- 222/222 unittest PASS (26.327 s), compileall PASS. Ověřena změna oběma směry, nativní mezerník, disabled, zachovaný stav při přepnutí tématu a unknown mount. verify_mount_switch skutečných widgetů PASS se stale-ack kontrolou a beze změny profilu. Screenshot980×640 potvrdil přepínače bez překryvů a odstraněný souhrn. Testovací okno ukončeno; herní vstupy neodeslány.

## 2026-10-12 — English interface

Completed translation of all app-owned UI text and runtime/validation diagnostics. Internal configuration values and user-defined profile/skill names remain unchanged. Updated all state comparisons and test expectations together. See docs/verification.md for full 222-test and real-widget verification evidence. No game input or user-profile writes performed.

## 2026-10-12 — Remembered mount switch

Mount state now restores the last known On/Off using application preferences, outside server profiles. Immediate declarations and completed automatic transitions persist; same-session reconfiguration preserves live mount state. Internal uncertain-input protection remains. README, buff spec, regression tests and widget verification updated together.

## 2026-10-12 — Numeric controls

Completed native numeric up/down stepping across calibration, timings and buffs. NumberRule shares canonical limits between validation and editor; unit-dependent interval bounds and dark/light arrow styles included. Verified with 226 tests, real-widget theme diagnostics and a native compact screenshot. See verification.md.

## 2026-10-12 — Setup documentation and launcher

README consolidated with current installation, requirements, source execution, verification, calibration/buffs/data and troubleshooting. Launcher renamed run.cmd with explicit native UAC request through tools/launch.ps1; cancelled requests abort. Application privilege guidance and repository ignore rules updated. See docs/verification.md for mocked UAC and environment verification.
