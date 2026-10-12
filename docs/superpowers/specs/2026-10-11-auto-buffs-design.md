# Automatické buffování mezi cíli

Uživatel požaduje intervalové buffování po dokončení metinu, se sesednutím, uspořádanými skilly s prodlevami a návratem na mount jen pro třídy, které z něj bojují. Potvrdil toggle Ctrl+G a požádal o přepínač mountu. Implementace rozšiřuje existující controller, bez dalšího režimu.

Jedna validovaná konfigurace BuffConfig v profilu: enabled (default false), fight_mounted, mount_shortcut (Ctrl+G), mount_wait_seconds (1.5), key_hold_seconds (.15) a až osm BuffSkill(shortcut, wait_seconds, mode, icon, inactive_icon, interval_seconds). Starší uložená data potřebují bezpečný default; další API ani legacy režim nevzniká.

Controller vstoupí do BUFFING pouze při stabilně nepřítomném panelu a po stávajícím settling. První cyklus při spuštění, za potvrzené nepřítomnosti panelu a alespoň třemi čerstvými snímky; vlastní interval každého běžného skillu od jeho úspěšného stisku a vlastní prodlevy. Čas pauzy se nezapočítá. Pouhé srovnání mountu do režimu boje proběhne před výběrem prvního cíle, také za potvrzené nepřítomnosti panelu. Klávesový krok je potvrzen až po úspěšném uvolnění vstupů, teprve pak začne prodleva. Během buffování se nekliká ani neotáčí; nově vzniklý panel běh pozastaví.

Worker je jediný vlastník BuffCycle pro aktuální identitu profile UUID + HWND + PID. Změna stejné konfigurace přes uložení ani clear/obnovení seznamu nesmí resetovat interval nebo mount. Změna buff konfigurace resetuje plán, zachová mount; nová identita začne s neznámým stavem mountu. Explicitní potvrzení z UI dovolí přepsat známý/nejistý mount. Nepotvrzená chyba přepínací zkratky znamená unknown; do ručního potvrzení nepovolí start s buffy.

UI: záložka Buffy se dvěma profilovými přepínači, samostatným aktuálním stavem mountu a okamžitým předáním změny přepínače bez Save, intervalem a odpočtem každého běžného skillu a zkratkou mountu, prodlevami a dynamickým seznamem nejvýše osmi skillů s přidáním, odstraněním a přesuny ↑/↓. Aktuální krok v existujícím stavovém pásu, odpočet a mount v záložce. Zkratky Ctrl/Shift/Alt + písmeno/číslo/F1–F12; F7/F8 rezervované. Podrobnosti a geometrie zpráv zachovány.

Ověření: config roundtrip a validace, controller target-end gating a foot/mounted sekvence, pauza/odpočet, runtime acknowledgement a unknown latch, per-key F8/focus cleanup, worker save/refresh/disable preservation, skutečný Tk editor s izolovaným profilem a zákazem herních vstupů. Živá odezva klienta a vizuální detekce mountu/cooldown nejsou součástí automatického důkazu.

Změna požadavku: zkratky se zadávají nahráním stisku do readonly pole po kliknutí. Jeden shared KeybindEntry pro mount i skilly; placeholder není součástí dat. Esc/focus loss/Tab zruší, Delete/Backspace smaže volitelný skill. Bez textového legacy režimu.

## Okamžité potvrzení mountu — 2026-10-11

Aktuální mount je samostatná UI deklarace, nikoli změna profilu. Změna přepínače ji automaticky odesílá, potvrzovací tlačítko bylo odstraněno. confirm_mount posílá UUID+HWND+PID a bool do vlastnícího workeru, který pozastaví runtime, přepíše mounted a vrátí ID potvrzení v Sample. UI potvrdí změnu až podle tohoto ID, nikoli starého vzorku. configure obnoví poslední zapamatovaný mount jen při nové identitě; stejné připojení zachová aktuální runtime stav. Počáteční deklarace initial_mounted byla z profilu odstraněna; starší pole se při načtení zahodí. Nové připojení z aplikace obnoví UISettings.last_mounted (bool, výchozí False); odpočet při změně zachován. Ovládání je prostá sekce bez vnořeného rámečku.

Klávesové kombinace používají jednu SendInput cestu: modifiers down, 100 ms guarded wait, primary down, nastavovaný hold, primary up, 50 ms guarded wait, modifiers up. Konečné uvolnění při přerušení v každém kroku. Uživatel potvrdil funkční fyzické Ctrl+G; nové časování v klientu je třeba živě ověřit, nelze ze SendInput úspěchu odvodit sesednutí.

## Kontrola přijetí kláves — 2026-10-11

Časování samo dle uživatele sesednutí nevyřešilo. Jeden keyboard transport nyní odesílá explicitní VK, s levým Ctrl/Shift/Alt. Zkratka ověřuje GetAsyncKeyState po modifier wait a po primary hold; odmítnutý stav znamená chybu, cleanup a mount unknown, žádné skilly. SendInput count ani held stav nejsou potvrzením reakce hry. Původní scan-only Win32 formát nebyl neplatný; zlepšení kompatibility musí projít živým testem. Uživatel další restart odložil.

## Aktuální keyboard transport — 2026-10-11

Předchozí kontrola hlásila nepřijatý Ctrl i po změně formátu. Nahrazena jedinou window-addressed PostMessage cestou pro zkratky a rotaci. send_key vyžaduje GameWindow a Alt context. Driver drží původní target pro všechny up události, včetně focus loss; reused HWND/PID odmítnut. WM_KEYDOWN/UP nesou scan a předepsané uvolňovací flags, Alt používá systémové zprávy. SendInput keyboard ani _confirm_keys už nepoužíváme. Globální key_held slouží pouze fyzickým vstupům a F8. Přijetí frontou nepotvrzuje reakci klienta; uživatel restart odložil, živé sesednutí musí být ověřeno.


## 2026-10-11 — Režimy skillů a aktivní ikony

Každý BuffSkill má mode refresh/inactive a volitelnou BuffIcon (validovaný PNG vložený do JSON profilu). BuffConfig.icon_region je společná ručně vybraná oblast aktivních ikon, oddělená od hotbaru. Starší uložené skilly zůstávají refresh. Neúplná kalibrace je přípustná jako draft, ale blokuje běh. Picker používá existující náhled a obdélníkovou interakci, nezasahuje do oblasti metinů; miniatura zobrazuje uložený výřez.

BuffDetector vrací ACTIVE/ABSENT/UNKNOWN: korelace >=0.90 znamená aktivní, <=0.65 vyžaduje tři čerstvé snímky pro nepřítomnost. Mezilehlý výsledek, černý snímek nebo nesprávná geometrie znamená nejistotu. Freshness respektuje nastavený sample_interval. BuffCycle kontroluje poslední pozorování před každým skillem; inactive odešle pouze ABSENT, refresh vždy. Přeskočené skilly mají viditelný důvod. Keyboard transport zůstává stávající fungující window-addressed cesta.


## 2026-10-11 — Úvodní cyklus a samostatný editor buffů

První resume nového BuffCycle nastaví due[index]=now pro každý běžný skill; pause/resume existujícího cyklu zachovává zbývající čas. Aktivní cíl se bezpečně dokončí před buffy. Úvodní kontrola čeká alespoň tři snímky i při panel.stable_frames=2. Další due[index] je čas úspěšného uvolnění vlastního skillu + jeho prodleva + jeho interval. Režim inactive nadále posílá pouze potvrzenou ABSENT.

BuffList vlastní jednu kolekci SkillRow draftů. Pořadí kolekce určuje pořadí runtime i JSON; přesun zachovává všechny údaje a miniaturu, odstranění zahodí celý řádek. U načtení se vytvoří pouze skutečně uložené skilly, žádné prázdné sloty. Přidání dovoluje maximálně osm řádků, prázdné zkratky se do profilu neukládají. Změna seznamu pozastaví běh a zruší rozpracovaný výběr ikony.

Mode.BUFF_AREA má vlastní geometrickou oblast v Calibration a používá plný náhled nezávisle na WORLD/PANEL. Lze ji kreslit, posouvat, měnit úchyty a mazat. Mode.BUFF_ICON vybírá nový výřez pouze uvnitř této oblasti; nedotýká se regionu metinů. Oblast buffů zůstává v náhledu označená i při výběru ikony. Původní WORLD picker pro buffy a paralelní pole UI byla odstraněna.

Změna velikosti snímku zruší oblast buffů i všechny pixelové výřezy ikon v kalibraci a UI draftech. Zkratky, režimy, pořadí a prodlevy zůstanou zachované; přepínací skilly blokují běh do nové kalibrace.


## 2026-10-11 — Dvojice vzhledů hotbaru

BuffSkill.inactive_icon je volitelný validovaný PNG v JSON, vedle icon (aktivní). Oba lze vybrat samostatně; detector porovnává každý ve vlastní pixelové velikosti. Pro hotbar s dvojicí známých vzhledů musí vítěz dosáhnout korelace >=0.90 a náskoku >=0.08; neaktivní vítěz vyžaduje tři čerstvé snímky. Nenalezení obou není neaktivita, podobné výsledky jsou UNKNOWN. Při konfiguraci pouze aktivní ikony zůstává explicitně detekce jejího zmizení, určená pro skutečné buffové ikony. Nejde o fallback při neúspěchu porovnání dvojice.

UI vlastní oba vzory v jednom SkillRow, ukazuje dvě miniatury jen u přepínacího režimu. Oba pickery používají volné tažení uvnitř společné oblasti. Nahrazení nebo smazání jednoho vzoru zachová druhý. Stejný aktivní/neaktivní obrázek UI odmítne a požádá o nový snímek; runtime nezávisle blokuje i nerozlišitelnou dvojici načtenou ze souboru. Změna rozlišení ruší obě ikony. Picker ověřuje platnost kalibrace až po zmrazení aktuálního snímku, které ji může zneplatnit.


## 2026-10-11 — Průběžná kontrola přepínacích skillů

Interval je pouze pro mode=refresh. BuffConfig.has_refresh/has_toggles slouží jako společná klasifikace pro scheduler, controller a UI. Bez běžných buffů nevzniká due/odpočet. BuffCycle._skill_indices sdílí výběr pro needed a begin: běžné skilly při due, přepínací jen při potvrzené ABSENT bez čekání na interval. Cyklus pouze přepínacích skillů nemění běžný deadline. Výběr drží konfigurované pořadí; před skutečným stiskem se stav znovu vyhodnotí.

Controller provádí kontrolu na bezpečné hranici před cílem, až po potvrzené nepřítomnosti panelu a settling. ACTIVE/ACQUIRING žádný buff nespouští. Před stejným dalším kliknutím je potlačen pouze přepínací skill, pro který už byla vyžádána klávesová akce: attempted_skills uchovává kanonické indexy skutečně vyžádaných skillů obou režimů a resetuje se při kliknutí na další cíl nebo start/resume. Ostatní skilly se dál kontrolují, včetně vypršení během animace běžného buffu. To zabrání opakované aktivaci stejného skillu před výběrem metinu, aniž by běžný nebo mount-only cyklus zablokoval dosud neprovedené přepínací skilly. Stav ACTIVE/UNKNOWN bez splatných běžných skillů a bez potřeby změnit bojový mount nevyvolává žádné klávesy. UI odpočet označuje běžné buffy a u toggle-only vysvětluje kontrolu mezi metiny.


## 2026-10-11 — Vlastní intervaly běžných skillů

BuffSkill.interval_seconds je 10–86400 pro refresh (default120), None pro inactive. BuffConfig nemá globální interval; Profile.from_dict jednorázově převádí starý interval do běžných skillů, již vlastní intervaly zachovává. JSON zapisuje jen nový formát. BuffCycle vlastní mapu termínů podle kanonických indexů a mapu zbývajících časů pro pauzu. Sample.buff_skill_seconds a BuffList zobrazují vlastní odpočty ve stejném pořadí. Agregovaný status používá nejbližší termín.

Splatné skilly a potvrzeně neaktivní toggly se seskupí do jedné uspořádané sekvence. Potvrzení klávesy plánuje vlastní deadline po vlastní prodlevě, nikoli po celé sekvenci. Každý skill smí být vyžádán jen jednou před dalším pokusem o cíl; krátký interval během jiné animace tak nezpůsobí nekonečné buffování. Evidence se resetuje při dalším kliknutí nebo resume. Pauza zachová každý čas samostatně.


## Jednotky intervalu — 2026-10-11

BuffList nabízí s/min vedle každého běžného intervalu. Přepnutí přepočítá aktuální číslo a zachová délku; prázdné pole zůstane prázdné. Chybné číslo zabrání přepnutí a zobrazí vysvětlení. Jednotka je pouze prezentační stav řádku; runtime a profil mají jedinou kanonickou hodnotu interval_seconds. Načtení zvolí min pro celé násobky 60 od jedné minuty, jinak s. Řazení přenáší celé řádky včetně jednotky. Limity10–86400s platí po převodu pro obě jednotky.


## 2026-10-11 — Symetrický výběr a opakování oblasti vzorů

BuffIcon obsahuje PNG a volitelnou normalizovanou Region původního výřezu. Profile.from_dict parsuje Region; starší PNG bez regionu fungují v detekci, tlačítko opakování oblasti je nedostupné do nové kalibrace. Oba vzory mohou být první a oba se kreslí samostatně tažením; vlastní velikosti detector podporuje přímo, bez škálování nebo druhé detekční cesty.

Každý vzor má samostatné × a tlačítko Stejná oblast jako druhý vzor. Znovu použije jeho souřadnice na aktuálním zmrazeném snímku, nikdy jeho PNG. Sdílené capture_buff_icon ověří oblast buffů a získá nový validovaný PNG; stejné fotky odmítne v obou směrech. Resize zneplatní oba vzory i jejich souřadnice. Smazání aktivního vzoru zachová neaktivní, ale blokuje běh do doplnění aktivního.


## 2026-10-11 — Oddělené stavy a reset odpočtu skillu

Přehled má samostatný stav běhu, popsaný limit/krok a čtyři metriky relace. Pevná výška řádku a zprávy brání skákání náhledu; rozložení ověřeno na1280×840 a980×640. Buffy mají oddělené Automatiky/Nastavení/Cyklus. Každý běžný skill ukazuje HH:MM:SS, vlastní textový stav a ↻; přepínací ukazuje Presence z runtime Sample.buff_states a nemá odpočet ani reset. UI žádný čas nepočítá, pouze formátuje kanonické snapshoty.

Uživatel vybral reset na nulu s použitím po dokončení metinu. Worker přijímá reset_skill_timer s UUID/HWND/PID, očekávanou BuffConfig a indexem běžného skillu; změněnou identitu/konfiguraci ignoruje. BuffCycle nastaví pouze jeho due/paused_remaining na nulu, ostatní termíny i mount zachová. Vstup stále posílá výhradně controller na bezpečné hranici, včetně ustálení obrazu; při pauze čeká na resume. UI blokuje reset u draftu, vypnutých buffů a před připojením. Reset statistik zachovává zobrazený deadline kroku i skill snapshot.


## 2026-10-11 — Automatický přepínač stavu mountu

Jsem na mountu posílá deklaraci při každé změně přímo workeru bez Save. IntVar používá pouze 1 mount a 0 pěšky; stavový text je On/Off. Poslední volba i potvrzené automatické přepnutí se ukládají do UISettings.last_mounted mimo profil. Během čekání na mount_confirmation_id UI nepřepisuje novou volbu starým vzorkem. Worker nadále kontroluje UUID/HWND/PID, pozastaví běh a vrátí potvrzení skutečně aplikované změny. Běh se automaticky neobnovuje.

BuffConfig.initial_mounted a jeho UI/profile binding odstraněny. Profile.from_dict zahodí staré pole, JSON ho již neobsahuje. BuffCycle začíná mounted=None. Stejná identita zachovává průběžný deklarovaný stav přes pauzu/uložení/refresh; nová identita z aplikace obnoví poslední uloženou volbu. Interní None je vyhrazen pro neurčitý výsledek chybného vstupu a nadále blokuje pokračování; UI přitom zachová poslední On/Off. Změna aktuálního mountu nijak nemění draft buffové konfigurace ani vybraný profil.


## 2026-10-12 — Témata a přehlednější editor

Schválený rozsah: sbalitelné pokročilé mount parametry, názvy skillů, přehlednější bloky, stavové štítky, miniatury kalibrace s hover zvýrazněním a dark mode.

UISettings(theme=light/dark) je validovaná preference aplikace v samostatném ui-settings.json. Theme.apply používá jednu paletu pro ttk styly, readonly/keybind pole, canvas okolí, listboxy, scrollbary, ikony včetně aktivní akce, spinner a tooltip. Původní snímek a význam detekčních barev zůstávají zachované. Preference tématu nepauzuje worker ani nemění profil.

Disclosure skrývá pouze uživatelské ovládání zkratky a prodlev mountu; ponechává skutečné proměnné a vstupní widgety. BuffSkill.name je volitelný validovaný text do60 znaků. SkillRow vlastní název spolu se zkratkou, intervalem a oběma ikonami; pořadí a mazání zůstávají jednou kolekcí. execution_config vynechává pouze názvy pro porovnání runtime plánu; worker při přejmenování aktualizuje text konfigurace a zachová termíny i mount.

Kalibrační miniatury vznikají z původního frame/template, používají cache podle snímku, regionů, vybrané masky, template a HSV. Hover má vlastní krátkodobý stav, nepřepisuje Mode ani geometrii. Přepínací skill bez aktivního vzoru/oblasti ukazuje Chybí kalibrace; štítky kombinují text s barvou, nespoléhají pouze na barvu.


## 2026-10-12 — Téma jako ikonové tlačítko

- Combobox a jeho StringVar/mapování odstraněny. Jediný toggle_theme přepíná light/dark, uloží preferenci a teprve po úspěchu aplikuje téma. Chyba zápisu ponechá původní téma a ikonu.
- Měsíc znamená přepnout na tmavé, slunce na světlé; dynamická nápověda akci pojmenuje. Ikony jsou součástí stávajícího generátoru a theme registry.
- 8 cílených unittest PASS, compileall PASS, verify_theme skutečných Tk widgetů PASS včetně invoke tlačítka oběma směry, uložené preference, nové ikony a zachovaných profilů/pixelů. README a diagnostické fixtures aktualizovány.


## 2026-10-12 — Přepínače a odstranění duplicitního souhrnu buffů

- Všechny čtyři checkboxy mají jednotný Switch.TCheckbutton: automatické buffování, boj na mountu, aktuální mount a detekce. Nativní ttk proměnné, callbacky, Tab/mezerník a disabled zůstávají zachovány. Paletové image indikátory jsou cached na vlastním Tcl rootu pro light/dark, on/off/unknown a disabled. Mount deklarace neznámého stavu explicitně používá alternate; bez vybraného okna je přepínač nepřístupný.
- Souhrn Automatiky/Nastavení/Cyklus odstraněn včetně StringVars a update větví. Uložení ukazuje jen footer; runtime stav horní přehled; vlastní detekce/odpočty zůstávají u skillů.
- 222/222 unittest PASS (26.327 s), compileall PASS. Ověřena změna oběma směry, nativní mezerník, disabled, zachovaný stav při přepnutí tématu a unknown mount. verify_mount_switch skutečných widgetů PASS se stale-ack kontrolou a beze změny profilu. Screenshot980×640 potvrdil přepínače bez překryvů a odstraněný souhrn. Testovací okno ukončeno; herní vstupy neodeslány.


## Mount preparation without a selected window

The Current mount state switch is always enabled. An offline edit updates UISettings.last_mounted immediately and sends no window-scoped worker command. Selecting a game window restores that preference into a new worker session. Attached-window edits still use the existing identity-checked acknowledgement and pause behavior. Samples from a previous window do not overwrite an offline declaration.
