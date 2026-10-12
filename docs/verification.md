# Aktuální ověření — profily a interaktivní editor


## 2026-10-11 — Přepínací buffy mezi metiny, běžné podle intervalu

- Interval je nově pouze pro mode refresh. Inactive skilly se vybírají při potvrzeném ABSENT na bezpečné hranici před dalším cílem, bez čekání na dlouhý deadline. ACTIVE/ACQUIRING buffy nepřerušují. Samostatná aktivace přepínacího skillu zachovává deadline běžných buffů. Toggle-only nemá interval ani odpočet; aktivní/nejisté ikony samy nevyvolají sesednutí.
- Jeden _skill_indices sdílí výběr pro needed/begin a respektuje konfigurované pořadí. Před stiskem se znovu ověří aktuální stav. Interval se nastaví pouze při dokončení cyklu, který zahrnoval splatné běžné buffy. Pause/resume zachovává běžný deadline.
- Nezávislá kontrola odhalila globální blokování všech togglů po běžném buffu. Nahrazeno attempted_toggles po jednotlivých kanonických indexech a zapisuje se až při vrácení klávesové akce konkrétního togglu. Skilly, které vyprší během běžné animace nebo jiného togglu, se mohou před dalším cílem ještě použít; již použité se před stejným cílem neopakují. Kliknutí na cíl nebo start obnoví tuto evidenci.
- 199/199 unittest PASS (24.214 s), compileall PASS, pip check clean. Regrese: obnova togglu mezi dlouhými intervaly, zachovaný běžný deadline, žádný odpočet/mount při active/unknown toggle-only, čekání na target end, vypršení během běžné animace, dva samostatné toggly bez opakování prvního a UI popisky. Tk fixture PASS pro toggle-only/mixed stav bez ukládání skutečného profilu nebo herního vstupu. Živý klient touto změnou netestován.


## 2026-10-11 — Aktivní a neaktivní vzhled hotbaru

- Uložená aktivní šablona uživatele měla proti přiloženým výřezům korelaci 0.67657 (neaktivní) a 0.71460 (aktivní). Oba výsledky byly UNKNOWN; log 22:33:55/58 potvrzuje odeslání pouze 3 a 4 po sesednutí. Převzetí raw nízkého prahu by nebylo bezpečné.
- BuffSkill.inactive_icon přidává volitelný neaktivní PNG stejné velikosti. Dvojice vyžaduje vítěze >=0.90 a odstup >=0.08; neaktivita má tři čerstvé snímky. Chybějící/nerozlišitelné podoby jsou UNKNOWN, bez fallbacku na prostou nepřítomnost. Detekce pouze mizející aktivní ikony zůstává explicitním nastavením bez inactive_icon pro skutečnou buffovou lištu.
- Přepínací řádek zobrazuje Aktivní ikona / Neaktivní ikona a obě miniatury. Neaktivní výběr drží velikost aktivního a ukotví se klikem na levý horní roh. Aktivní recapture ruší neaktivní; × dovoluje neaktivní vzor odstranit. Stejné vzory UI odmítá s výzvou obnovit snímek. Pořadí, uložení a změna rozlišení zacházejí s oběma vzory společně.
- Nezávislá kontrola našla resize mezi kontrolou a zmrazením snímku; kontrola regionu a aktivní ikony přesunuta až po zmrazení. Regrese pokrývá oba picker stavy. Při nové velikosti se picker nezapne s vymazanou kalibrací.
- 193/193 unittest PASS (23.125 s), compileall PASS, pip check clean. Dva lokální 35x35 hotbarové výřezy z uživatelových snímků v tests/data; aktivní/neaktivní rozlišení, absence obou, identická dvojice, serializace, rozměry, startup sekvence, miniatury a řazení ověřeny. Neaktivní startup odesílá Ctrl+G/2/3/Ctrl+G, aktivní Ctrl+G/3/Ctrl+G v controller testu.
- Tk fixture PASS: výběr obou stavů, automatická 35x35 velikost neaktivního vzoru, dvě miniatury, zachovaný world region, uložení, reorder a klasifikace. Fixture zakazuje herní vstupy, skutečný profil se nezměnil. Živá reakce klienta na tuto detekci nebyla ověřena; uživatel musí po restartu znovu zachytit oba vzhledy z aktuálních snímků.


## 2026-10-11 — Buffy při startu, dynamické řádky a samostatná oblast

- Úvodní BuffCycle je splatný při prvním startu; controller čeká na bezpečně nepřítomný panel a alespoň tři čerstvé snímky. Aktivní cíl nejprve dokončí. Interval začíná po plném dokončení sekvence; pause/resume zachovává zbývající čas. Aktivní a nejisté přepínací skilly zůstávají přeskočené.
- BuffList nahradil paralelní pole a osm předem vykreslených slotů jednou kolekcí celých SkillRow draftů. Přidání, odstranění, přesuny ↑/↓, zachování všech parametrů i obrázku, načtení pouze uložených řádků. Maximum osm zůstává kanonickou validací profilu. Nové widgety se zapojují do existujícího scrollování.
- Dedikované Mode.BUFF_AREA a BUFF_ICON: nezávislý výběr v celém snímku, vlastní popisek a úchyty, posun/resize/new/delete oblasti buffů. Výřez ikony musí být v oblasti buffů. Původní WORLD sdílení odstraněno.
- Nezávislá kontrola našla staré buffové PNG po změně rozlišení. Opraveno zrušením regionu a ikon v Calibration i UI, při zachování pořadí, zkratek, režimů a prodlev. Neúplná detekce blokuje běh.
- 185/185 unittest PASS (19.898 s), compileall PASS, pip check clean. Tk fixture PASS: oblast mimo WORLD, úprava oblasti, zachovaný svět, odmítnutí ikony mimo oblast, miniatura, přesun/odstranění a uložení. Herní vstupy zakázané ve fixture; skutečné profily nezměněny. Živý start buffů v klientu v této změně neověřen.


## Režimy skillů a detekce aktivních buffů — 2026-10-11

- Uživatel potvrdil funkční klávesy v klientu před touto změnou. U každého skillu přidán refresh/inactive režim, výběr aktivní ikony v náhledu a miniatura. Společná oblast aktivních ikon odděluje buffy od hotbaru. Ikony jsou validované PNG v JSON profilu, bez nových externích souborů. Neúplný draft lze uložit, běh blokuje readiness.
- Detektor kontroluje barvenou šablonu v určené oblasti. Aktivní nebo nejisté skilly se neposílají; nepřítomnost vyžaduje tři čerstvé snímky. Poslední stav se vyhodnocuje před každým skillem, přeskočení uvádí důvod. Černé snímky a jiné rozlišení neposkytují důkaz nepřítomnosti.
- Nezávislá kontrola odhalila a oprava pokryla validní 2s sample interval a Delete/mazání oblasti během výběru buffu. Escape zruší picker. Původní oblast metinů se výběrem buffu nemění.
- 178/178 unittest PASS (21.976 s), compileall PASS, pip check clean. Regrese pokrývají serializaci, staré profily, přesun ikony, hotbar mimo oblast, tři absent snímky, staré snímky, 2s snímání, neplatné obrazy, rozhodování sekvence, poslední pozorování, readiness, runtime a bezpečnost pickeru.
- Skutečný Tk App s disposable fixture PASS: oba výběry obdélníkem, miniatura, mode/ready, uložení, Delete a Escape, původní world region zachován. Herní vstupy fixture zakazuje; skutečný profil nebyl změněn. Konkrétní aktivní ikony serveru nejsou živě kalibrované ani ověřené; spolehlivost závisí na výběru stálého výřezu a oblasti.

## Dlouhé intervaly buffování — 2026-10-11

- Limit BuffConfig.interval_seconds zvýšen z 3600 na 86400 sekund (24 hodin); čtyřhodinový interval 14400 je validní. Stejná kanonická validace platí pro editor i načtení profilu. UI tooltip vysvětluje rozsah, převod 240 minut a aktivní čas. Skutečný profil nezměněn.
- 167/167 unittest PASS; compileall PASS. Regrese ověřuje 14400 sekund při profile JSON roundtrip, spuštění, pause/resume a správný okamžik due. Žádné testy s reálným čekáním čtyř hodin.
- Rozlišení obnovitelných a přepínacích skillů je zatím návrh, není implementované: přepínací skill potřebuje detekci aktivního buffu z obrazu, aby opakovaný stisk nezpůsobil vypnutí.

## Klávesové zprávy do původního herního okna — 2026-10-11

- Uživatelův nový snímek a log 19:19:00 prokazují: SendInput přijalo levý Ctrl (0xa2/scan 0x1d), ale GetAsyncKeyState(0x11) po 100 ms stále false. Zastavení před G zabránilo dalšímu předstírání proběhlého cyklu. Není to doklad konkrétního interního důvodu, proč klient/global input nereaguje.
- Klávesy převedeny na jedinou window-addressed PostMessage cestu, obdobně jako fungující myš. InputAPI.send_key bere původní GameWindow; driver drží tento target do konečného cleanup. Kontrola identity HWND/PID při down i up, foreground při down, up dovoleno po focus loss; zaniklé/reused okno bezpečně ukončí ownership bez zprávy do cizího procesu. Neúspěšný PostMessage vrací WinError a přeruší runtime před skilly.
- WM_KEYDOWN/UP mají VK, scan v bits16–23, repeat1 a uvolňovací bits30/31. Alt používá WM_SYSKEYDOWN/UP a context bit29. Typy a reference: https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-keydown a https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-keyup. Původní SendInput keyboard a globální post-send kontrola odstraněny; GetAsyncKeyState zůstává pro skutečně držené klávesy a F8. Myš/cursor se nemění. ABI KEYBDINPUT zůstává členem Win32 INPUT union pro správnou deklaraci celé struktury, není keyboard transportem.
- Přijetí zprávy do fronty není důkaz zpracování kombinace klientem ani sesednutí. Log to výslovně uvádí. Chování klientů používajících jiné zdroje stavu kláves zůstává k živému ověření, nepřidává se skrytý fallback nebo injekce do procesu.
- 166/166 unittest PASS, compileall PASS, pip check clean. Regrese: payload Ctrl/G a Alt, pořadí a cleanup, unobservable global state, odmítnutá zpráva, focus loss, reused HWND a runtime unknown/žádné další skilly po chybě. Při úpravě testů chybný implicitní cp1252 read způsobil dvě textové test failures; obnovena UTF-8 česká očekávání a ověřena celá sada znovu.
- Uživatel opět odložil restart. Nový backend nebyl živě spuštěn v jeho klientu, účinné sesednutí proto neověřeno.

## Ignorované Ctrl+G po restartu — 2026-10-11

- Nový log 19:09:59 prokazuje Ctrl+G (17,71), 150 ms hold + prodlevy a dokončení bez reakce hlášené uživatelem. Předchozí změna časování problém nevyřešila. Read-only kontrola skutečných oken: klient PID 38412 i bot PID 43460 elevated True; VK_CONTROL mapuje na 0x1d a G na 0x22. Oprávnění ani toto mapování proto nevysvětlují selhání.
- Computer Use zachytil klienta a jednorázově poslal F8, Ctrl+G a i. Snímky neukázaly sesednutí ani zavření inventáře. Tento helper nemá prokázanou shodnou úroveň oprávnění s elevated botem; není to důkaz, že všechny správně privilegované události klient ignoruje. Inventář, herní stav a skutečný profil se záměrně neměnily. Kontrolní klávesy neměly pozorovatelný efekt.
- Keyboard SendInput změněn ze scan-only událostí na explicitní VK (levý Ctrl/Shift/Alt), stejný down/up transport pro zkratky a rotaci. Žádné druhé backendy nebo automatické fallbacky. Původní scan-only formát je podle Win32 platný; změna formátu je kandidátní oprava kompatibility, nikoli prokázaná příčina chyby klienta. Reference: https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput.
- Nově GetAsyncKeyState potvrdí modifikátory před hlavní klávesou a celou kombinaci během hold před uvolněním. Při nepotvrzeném stisku SafetyError, konečné uvolnění kláves a unknown mount; nepokračuje se na skilly. Log zaznamená VK/scan/down a observed held stav. Potvrzený Windows stav stále není vizuálním důkazem sesednutí.
- 163/163 unittest PASS; compileall PASS; pip check clean. Nové regrese ověřují skutečný ctypes event payload, nepřijatý modifier/primary, cleanup a runtime zastavení před skilly včetně odmítnutí F7 při unknown mount. FakeAPI nyní pravdivě modeluje key down/up stav; worker fixture používá stejnou implementaci.
- Uživatel výslovně odložil restart aktualizované verze. Živé ověření nového transportu v klientu zůstává neprovedené; nelze tvrdit, že Ctrl+G ve hře už funguje.

## Okamžité potvrzení mountu a časování kombinace — 2026-10-11

- Předešlé potvrzení čekající na Save nahrazeno jediným worker příkazem confirm_mount pro UUID profilu + HWND + PID. Stav se mění hned po zpracování příkazu, běh se pozastaví, odpočet ani konfigurace se neresetují. Sample vrací ID potvrzení, UI čeká na skutečné přijetí workerem; staré samples nesmějí potvrzení odklepnout. Stale HWND/PID požadavky nemění mount jiného klienta.
- Aktuální stav má samostatnou UI proměnnou, nepatří mezi profilové checkboxy a nezpůsobuje dirty profil. Odstraněny mount_override a configure(mount_state); není souběžná potvrzovací cesta přes Save. Prostá sekce místo LabelFrame; aktuální stav viditelný i při vypnutých buffech. Stávající initial_mounted v uložených profilech zůstává počáteční deklarací pro nové připojení, potvrzení ji nemění.
- V reálném logu 18:58:59–18:59:15 byly mount/skill akce označené completed bez chyb Windows, ač uživatel sesednutí neviděl. Kombinace dosud stiskla Ctrl a G bez prodlevy. Nově modifikátory drženy 100 ms před hlavní klávesou a 50 ms po jejím uvolnění, vlastní stisk zachovává nastavovanou délku. Log uvádí VK tuple a délku. Jde o opravu časování a ověřitelnou hypotézu o input polling klienta; skutečná příčina ignorování a sesednutí v klientu nebyly živě potvrzeny.
- 159/159 unittest PASS, compileall PASS, pip check clean. Pokryto okamžité potvrzení bez Save/dirty změny, odmítnutí stale PID, zachování timeru, pořadí Ctrl/G a waits, cleanup při F8 v obou prodlevách. Celá izolovaná Tk App přijala worker acknowledgement bez uložení, zobrazila pěší stav a zachovala nastavení buffů. Při prvním testovacím pokusu byl zapnutý draft bez skillu; schema ho správně odmítlo, opraven pouze fixture scénář. Žádné herní vstupy ani změny skutečného profilu. Reakce klienta na nové časování Ctrl+G zůstává k ověření po restartu.

## Oddělené potvrzení mountu a stav rozpracovaných buffů — 2026-10-11

- Potvrzení má vlastní skupinu Stav mountu a explicitní tlačítko Potvrdit stav mountu. Lokální zpráva říká, zda uživatel potvrdil mount/pěší stav, a že se použije až po uložení. Nepotvrzuje zapnutí buffování ani způsob boje.
- Stav buffování reaguje ihned na rozpracovaná nastavení. Neuložené buffy mají přednost před worker sample, po uložení se zobrazí čas běhu. Při změně profilu/okna a uložení se zahodí starý zobrazovaný sample; kanonický BuffCycle workeru se nemění.
- 157/157 unittest PASS, compileall PASS, pip check clean. Dvě regrese ověřují změny zapnutí/vypnutí vůči starým samples a samostatné potvrzení mountu. Celá izolovaná Tk App ověřila text po změně přepínače i potvrzení a rodiče tlačítka v mount skupině. Bez herních vstupů a změn skutečných profilů; vizuální screenshot ani chování hry se v tomto kroku neověřovaly.

## Zachytávání keybindů — 2026-10-11

- Všechna pole mount/skill nahrazena jedním KeybindEntry. Kliknutí/zaměření zahájí nahrávání a pozastaví worker. Pole readonly, žádná souběžná cesta pro ruční textový zápis. Samostatný display StringVar drží instrukci; canonical profilová proměnná obsahuje pouze platnou zkratku nebo prázdný skill.
- Windows VK + modifier state zachová klávesy číselné řady i se Shift/CZ rozložením. Ctrl/Shift/Alt, písmena, čísla a funkční klávesy používají společnou hotkey validaci. F7/F8 odmítnuté, samotný modifier neukončí nahrávání. Esc/FocusOut/Tab zachovají hodnotu, Delete/Backspace smaže pouze volitelný skill. Zachycené Ctrl+Z nepropadne do editorové historie.
- 155/155 unittest PASS; compileall PASS; pip check clean. Pět skutečných Tk/pure regresí včetně jednorázového commitu, modifiers/Shift-digit/Alt, rezervací, focus loss, smazání, načtení profilu a potlačení editorové zkratky. Tk virtuální Alt-KeyPress-g ověřen jako Alt+G.
- Celá izolovaná App: nalezeno devět recording polí, virtuální Tk klávesy nastavily mount Alt+G a skill F4; settings_profile vrátil odpovídající typed config, Delete odstranil skill. Testovací vlastní okno/thread ukončeno, herní vstupy a skutečné profily nezměněny. Nejde o ověření fyzické klávesnice uživatele nebo úspěchu skillu ve hře. Nový kód vyžaduje restart bota.

## Automatické buffování a mount — 2026-10-11

- Uživatel potvrdil toggle Ctrl+G a přepínač boje na mountu/pěšky. Nová záložka Buffy, typed BuffConfig/BuffSkill, pořadí až osmi skillů a prodlevy, interval aktivního běhu. První interval od startu, další od dokončení sekvence. Starší profily buffování vypnuté.
- Controller čeká na potvrzený konec panelu a settling, před dalším klikem sesedne podle potřeby, po skillu čeká, nasedne pouze při fight_mounted. Také počáteční srovnání mountu probíhá mimo aktivní cíl. Runtime potvrzuje krok až po uvolnění kláves; při chybě togglu mount unknown a F7 nesmí pokračovat bez výslovného potvrzení. Klávesové vstupy mají stávající F8/focus/geometry guards a konečné uvolnění.
- Nezávislý review našel reset intervalu přes UI Save/Start a ztrátu mountu při clear/refresh a disable/re-enable. Opraveno jedním worker-owned BuffCycle pro profile UUID+HWND+PID; běžné uložení/refresh uchová interval i mount včetně unknown. Nastavení buffů změněné resetuje interval, uchová mount. Explicitní potvrzení může mount změnit; nová identita použije deklarovaný stav. Focused re-review potvrdil obě opravy, nezávisle 7/7 worker testů PASS.
- Finální 150/150 unittest PASS, compileall PASS, pip check clean. Pokryto pořadí target→mount→skills→mount→click, foot boj bez zbytečného togglu, panel během buffování, prodlevy/ack, pauza, schema roundtrip, staré profily, rezervace F7/F8 a modifier cleanup při F8/focus loss. UI potvrzení mountu se počítá jako pending změna i při stejném profilu.
- Celá izolovaná Tk App přes fixture: třetí záložka a typed parsování polí/skillů fungují, bez zápisu skutečného profilu a bez herních vstupů. Pokus o Computer Use aktivaci fixture selhal; následný capture obsahoval foreground hru, proto jej nepovažujeme za obrazový důkaz nového UI. Fixture procesy ukončeny. Odezev konkrétního Verzum klienta na Ctrl+G/F1/čísla nebylo živě ověřeno; úspěšný SendInput není vizuální potvrzení mountu ani skillu. Nový kód vyžaduje restart uživatelova bota.

## Rozsah sidebarového scrollbar — 2026-10-11

- Reprodukován posun krátkého obsahu s plným scrollbar: canvas s kratším scrollregion dovoloval záporný offset (-180 px). Scrollregion má nyní minimálně výšku viewportu, krátký obsah se ukotví nahoře a scrollbar je neaktivní. Šířka sidebaru se nemění.
- Další reprodukce: zmenšení obsahu mimo viditelnou část nevygenerovalo Configure canvas-window; zůstával starý rozsah. Sledovány změny geometrie vnořených widgetů, požadovaná výška se synchronizuje po idle. Kolečko funguje i nad prázdnou plochou canvasu; jednotné bindování patří do scroll_content.
- 134/134 unittest PASS, compileall PASS, pip check clean. Tři skutečné Tk regrese: plný rozsah neumožňuje posun, zmenšení obsahu vrací offset na nulu, kolečko/thumb/resize odpovídají rozsahu.
- Celá izolovaná App, obě notebook záložky: Kalibrace po posunu yview=(0,1), offset=0; Nastavení při overflow yview shodné s hodnotou scrollbar a pozitivní offset. Bez herních vstupů, skutečné profily nezměněny; běžící aplikace vyžaduje restart.

## Spinner uvnitř tlačítka Snímek — 2026-10-11

- Opraveno umístění podle uživatelské připomínky: animovaný 16px spinner nahrazuje ikonu obnovení přímo v tlačítku. Vedlejší progressbar a jeho rezervovaný prostor odstraněny. Jedna LoadingButton spravuje animaci a obnovu původní ikony/stavu, callback se ruší i při zániku widgetu.
- 131/131 unittest PASS, compileall PASS, pip check clean. Skutečný Tk test ověřuje změnu obrázku v čase, návrat původní ikony, disabled/enabled a shodnou geometrii.
- Celá izolovaná App potvrzuje vnitřní ikonu při žádosti, návrat po timeoutu a stejnou geometrii tlačítka i canvasu. Žádné herní vstupy; uživatelova běžící aplikace vyžaduje restart. Starší záznamy o vedlejším loaderu jsou historické.

## Odstranění samostatného testu kalibrace — 2026-10-11

- Na žádost uživatele odstraněno tlačítko Test kalibrace, stav test_mode, přechody do/z testu a preview_only z API workeru a runtime. Jedna runtime cesta: zastavený náhled detekuje bez vstupů; Spustit nebo F7 aktivuje běh. Zachovány Detekce, informace o shodě panelu, čerstvost snímku, statistiky a F8.
- Odstraněny testy pouze pro zrušený režim; zachovány a upraveny regrese zastaveného náhledu bez vstupů, spuštění a zachování statistik při uložení. 130/130 unittest PASS, compileall PASS, pip check clean.
- Celá izolovaná Tk App: chybí tlačítko i stav testu, přítomny Spustit/Detekce, vykreslení při zapnutí/vypnutí detekcí a skóre panelu fungují. Fixture zakazuje herní vstupy; skutečné profily neupravovány. Živý běh touto změnou netestován, uživatelův proces vyžaduje restart. Starší zmínky o testovacím režimu níže jsou historické.

## Stabilní stavové zprávy a načítání snímku — 2026-10-11

- Reprodukce ve skutečných Tk widgetech: objevení zprávy o popředí posunulo prostor náhledu o 25 px. Automatické pack/pack_forget a zalamování zprávy odstraněno; důvod i spodní informace používají jeden společný řádek s pevnou výškou. Celý aktuální text je v tooltipu, důvod také přes informační tlačítko.
- Tlačítko Snímek během žádosti neaktivní; indikátor běží v trvale rezervovaném prostoru vedle něj. Úspěch, timeout, chyba aktivace a změna kontextu jej zastaví. Čekající callback se při přerušení ruší; opakovaná žádost za běhu se ignoruje.
- 134/134 unittest PASS; compileall PASS; pip check clean. Regrese ověřují neměnnou geometrii pro chybu, dlouhý/víceřádkový i prázdný text, dokončení a zrušení načítání.
- Celá izolovaná App z diagnostics/ui_fixture.py ověřena přímo přes Tk geometrii bez herních vstupů: canvas (x=0,y=86,width=1006,height=537) beze změny během všech zpráv, loaderu i timeoutu. Nejde o obrazovou kontrolu uživatelovy běžící aplikace. Její kód se aktualizuje po restartu; skutečné profily nebyly změněny.

## Kompaktní UI — 2026-10-11

- Uživatel před úpravou UI potvrdil, že adresované kliknutí vybírá a postupně ničí metiny. Je to jeho živé potvrzení, ne automatický test zničení. Tato úprava nemění game-input backend ani controller.
- Implementována celá schválená úprava: kompaktní profil/okno/uložení/start, jeden Start/Pauza, jednořádkové statistiky s tooltipy a rozbalitelnými podrobnostmi, kreslené ikonové nástroje, zoom −/+ a přizpůsobení, sbalitelný sidebar, kontextové ovládání kalibrace, přepínač detekčních rámečků a pravdivý stav snímku. Nastavení seskupeno podle detekce/panelu/časování. Zkratky a historie zachovány.
- 131/131 unittest PASS, compileall PASS, pip check clean. Nové testy ověřují start/pauzu včetně arm, ukotvení zoomu a zákaz během tažení, stárnutí snímku, obnovení oken bez shody a chybu workeru při zmrazeném snímku.
- Nezávislý review: opraven zbylý stav running/armed po odstranění vybraného okna a skryté chyby workeru během frozen kalibrace. Detail sleduje reason i status; zmrazení drží pouze obraz, nikoli důvod chyby.
- Computer Use: izolovaná skutečná Tk aplikace diagnostics/ui_fixture.py, bez registrace globálních zkratek a se zákazem všech game inputs. Ověřeno kompaktní rozložení, vybraný krok a maska/seznam, sbalení/obnovení sidebaru, zoom tlačítkem 125 %, vypnutí zelených detekcí. Náhled při 1280×840 má cca 570 px na výšku místo předchozích cca 330 px. Ikonová tlačítka skutečně zmenšena po první obrazové kontrole. Callback log diagnostics/compact-ui-error.log bez chyb.
- Fixture používá pouze diagnostics/compact-ui-fixture-data; skutečný profil Verzum ani běžící bot nebyly přepsány. Pokus o změnu velikosti okna nativním tažením nezměnil velikost, proto minimum 980×640 není obrazově potvrzené. Závěrečné opravy chybových přechodů potvrzeny testy; v otevřené fixture byly načteny předchozí callbacky. Nový UI kód vyžaduje restart uživatelova bota.

## Adresované kliknutí do klienta — 2026-10-11

- Uživatel fyzickou myší potvrdil výběr, panel i útok; botova SendInput stisknutí po restartu byla bez reakce. Přesná příčina jejich nedoručení není známá.
- Veřejný klasický klient explicitně obsluhuje WM_LBUTTONDOWN/UP a předává klientské souřadnice do OnMouseLeftButtonDown/Up: https://github.com/NakiuS/Metin2Client/blob/master/source/UserInterface/PythonApplicationProcedure.cpp . Není to ověření konkrétní binárky Verzum.
- Nahrazena pouze cesta myších tlačítek: PostMessageW do vybraného HWND s klientskými souřadnicemi, MK_LBUTTON při down a nulovým stavem při up. Původní SendInput mouse-down/up odstraněn, bez přepínače backendů a bez automatických fallbacků. Pohyb a klávesy nadále používají SendInput. Oprávnění, popředí, geometrie, kurzor a F8 se kontrolují před akcí a během držení; up se adresuje původnímu oknu i při ztrátě zaměření.
- PostMessage ověřuje návrat a loguje message/ HWND/ souřadnice, případně WinError. Před odesláním se ověřuje původní PID okna. Přijetí zprávy do fronty není důkaz výběru; panel zůstává jediným potvrzením výběru. Dokumentace: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-postmessagew .
- 123/123 unittest PASS před nezávislou kontrolou: přesné zprávy a souřadnice, odmítnutí neplatných bodů/popředí, chyba OS, původní adresát při uvolnění po focus loss a integrace runtime/worker. Přesné výsledky závěrečných kontrol jsou zapsány níže.
- Živý klient po této změně dosud neověřen; otevřený bot má stále předchozí kód. Vyžaduje restart jako správce a krátký běh s potvrzením panelu, útoku a F8. Nezvyšovat oprávnění automaticky, neobcházet UAC.
- Závěrečná kontrola: 125/125 unittest PASS, compileall PASS, pip check clean. Nezávislý review našel úklid při zániku/převzetí HWND během hold: zaveden WindowGoneError odlišený od ostatních chyb a bezpečné ukončení vlastnictví myši bez odeslání do náhradního okna. Regrese ověřují opakovaný úklid, zákaz down do zaniklého okna a up při pouhé ztrátě popředí.

## Kliknutí a dostupnost stavu myši — 2026-10-11

- Aktuální sada: 120/120 unittest PASS, compileall PASS, pip check clean.
- Přidány minimální rozměry kandidáta, konfigurovatelné ustálení kurzoru a délka stisku. Pohyb používá jedinou relativní SendInput cestu, měřenou korekci zrychlení a kontrolu zaměření/F8 před každou událostí; limit pohybu je nyní 500 ms. Stisk a uvolnění nepřidávají další pohyb.
- Review odhalil oscilaci při dvojnásobném zrychlení a chybějící guard jednotlivých pohybových kroků; obojí opraveno a pokryto regresními testy.
- Uživatelův běh v diagnostics/run.log v 16:15:25 dosáhl bodu client=(1436,540), screen=(1436,571). SendInput nehlásil odmítnutí; běh zastavila dodatečná podmínka GetAsyncKeyState po 250 ms. Neprokazuje to doručení události hernímu klientovi.
- Tato podmínka odstraněna: GetAsyncKeyState může podle dokumentace Microsoftu vrátit nulu při nedostupném přístupu mezi procesy. Stav je nyní jen diagnostický údaj po nastavené době stisku. Potvrzení výběru nadále provádí panel cíle, nikoli návrat SendInput nebo globální stav myši. Test nejprve reprodukoval chybné zastavení, po opravě ověřuje celý hold a uvolnění i při trvale nulovém stavu.
- Zdroj: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getasynckeystate
- Po této poslední změně není živý výběr ve Verzum potvrzen. Již běžící proces nenačte upravený Python kód; nutný restart launcheru a omezený živý test. Nelze zatím tvrdit, že byla odstraněna původní příčina ignorovaného kliknutí klientem.
- Navazující skutečný restart: log 16:23:10 a 16:23:30 uvádí přijatý mouse-down, dokončený hold/up a async state=False; uživatel potvrzuje kurzor bez útoku. Read-only kontrola současných procesů: hra PID 29516 elevated=True, bot PID 37088 elevated=True. Klient má počátek (0,31), velikost 2560×1354; bod (1277,475) v klientu odpovídá viditelnému kameni na obrazovce (1277,506). INPUT má 40 bajtů, union offset 8, MOUSEINPUT 32 bajtů a down flag 2. Tím není prokázáno doručení do klienta.
- Computer Use: jeden klik (1277,506) neotevřel panel; kvůli rozdílným oprávněním nástroje jej nelze považovat za reprodukci botova elevated vstupu. Vyžádán kontrolní fyzický klik při zastaveném botu. Bez tohoto srovnání není potvrzené blokování klientem ani vhodná náhrada vstupní cesty. Žádné další změny backendu naslepo.

2026-10-11: nový režim jednotlivých kliků nahrazuje frontový režim. Předchozí záznamy níže jsou historické.

- 108/108 unittest PASS po závěrečné kontrole. Pokrývají profily, atomické ukládání, barvy/masky, panelové přechody, guarded klik, oprávnění, Worker shutdown, geometrii editace, historii, zoom, změny profilů a test bez vstupů.
- Compileall PASS; pip check: No broken requirements found.
- Computer Use: skutečné okno nového editoru na izolovaném testovacím snímku, zablokované herní vstupy. Existující oblast se přesunula se zachovanou velikostí 1021×571 px, souřadnice se aktualizovaly a objevil se stav Neuložené změny. Rohový úchyt změnil oblast na 957×522 px. Aktivní krok i oblast jsou modré, stavy kroků a pokročilá nastavení jsou oddělené.
- Testovací fixture ukládá data pouze pod diagnostics/ui-fixture-data; nemění skutečnou kalibraci Verzum.
- Živý herní útok a pokračování na dalším metinu zůstávají neověřené. Při poslední kontrole hra byla elevated=True a bot elevated=False; získání snímku z tlačítka odmítla kontrola oprávnění. Nový editor neobchází aktivaci ani UAC.

- Native potvrzení po opravách: nová Maska 2 vytvořena tažením a označena; uložený profil načten zpět se dvěma maskami, template_region a PNG šablonou. Screenshot diagnostics/editor-final.png.
- Fresh review: opraven pád náhledu nové masky, zavedeno potvrzení nepřítomnosti panelu i při hledání/rotaci a priorita F8 nad souběžným F7. Každý nález reprodukován testem před opravou. Zrušení nového obdélníku tažením zpět do počátku také testováno.

## Historie, zoom a test kalibrace

- Ctrl+Z / Ctrl+Y a tlačítka Zpět / Znovu: 32 izolovaných stavů kalibrace včetně PNG, nové úpravy ruší redo. Návrat k uloženému stavu odstraní příznak neuložených změn.
- Jeden Viewport převádí souřadnice pickeru, oblastí i úchytů; zoom kolem kurzoru 100–800 %, posun prostředním tlačítkem, reset Celý snímek. Render vytváří jen bitmapu velikosti náhledu a opakovaně nepřepočítává detekci při posunu stejného snímku.
- Test kalibrace používá preview_only runtime: odmítá F7, arm i request_start. Skóre a detekce pokračují; nedostupný snímek označuje náhled jako neaktuální. Ukončení obnoví zastavený runtime vybraného okna.
- Neuložené změny: Ano / Ne / Storno před přepnutím, vytvořením profilu a zavřením. Neúspěšné uložení přechod zablokuje.
- Fresh statická kontrola našla dvě Important vady: výběr profilu po uložení během přepnutí a odpojený runtime po ukončení testu. Obě reprodukovány testem před opravou; žádný další review loop.
- Computer Use v izolovaném okně: zoom 125 %, tažení oblasti při zoomu, Ctrl+Z/Y, reset 100 %, test s rozpoznaným panelem (skóre 1,000), vypnuté spuštění, ukončení testu a dialog při zavírání. Escape ponechal editor s úpravami otevřený. Screenshot diagnostics/editor-tools-final.png.
- Fixture data pouze diagnostics/editor-tools-fixture-data; herní vstupy odmítá testovací API. Posun prostředním tlačítkem je automaticky ověřený v modelu souřadnic, nikoli nativním tažením: Computer Use podporuje tažení pouze levým tlačítkem. Živý útok zůstává neověřený.

## Přehled běhu a statistiky

- 115/115 unittest PASS, compileall PASS, pip check clean. Přidán Session vlastněný workerem a neměnné Statistics v Sample; UI nepočítá vlastní čas ani události.
- Aktivní čas zahrnuje běh a vstupy, vylučuje pauzu/test; dokončení je stabilní zmizení potvrzeného panelu. Klik se počítá po úspěšném odeslání mouse-down. Nepotvrzené/timeout pokrývá limit panelu nebo aktivního cíle.
- Uložení stejného profilu, pauza a test statistiky zachovají. Změna profilu/HWND je resetuje; ruční reset nemění bezpečnostní limity ani fázi. Worker test ověřuje zachování, reset a změnu okna.
- Fresh review: opraveno uzavření posledního aktivního intervalu při cancel, započtení doručeného kliku přerušeného během hold a odmítnutí opožděného neúspěšného snímku z jiného profilu/okna. Regrese pokryty testy. Celková chyba workeru zachová poslední statistiky v UI.
- Computer Use: izolovaný editor s novým přehledem, důvodem čekání, čtyřmi metrikami, diagnostickým režimem a resetem. Screenshot diagnostics/session-overview.png. Testovací API blokuje herní vstupy; zobrazené nulové statistiky nejsou živý farmovací výsledek. Nenulové hodnoty a odpočet ověřeny automaticky; živý útok nadále neověřený.

## Historické ověření původního režimu


## Automatické a obrazové kontroly

- 2026-10-11: 48 testů unittest prošlo v izolovaném `.venv`. Kontrolují konfiguraci, omezení všech akcí, neměnné označení, detekci a stabilitu paprsků při různých intervalech, ztrátu zaměření/geometrie, přerušení a uvolnění vstupů, systémové události startu, ustálení popředí a skutečný Worker shutdown během drag proti fake Win32 hranici.
- `python -m compileall -q metin_bot tools tests` prošlo.
- Uživatelův snímek s paprsky: nalezeny dva paprsky; přerušované segmenty druhého se slučují. Žluté číslo 3063 není označeno jako paprsek.
- Uživatelův snímek s modrou oblastí výběru: žádný žlutý paprsek.
- Read-only Windows kontrola mimo sandbox nalezla správné Verzum okno; v omezeném sandboxu nebylo přístupné přes dotaz na proces.
- Aplikace byla otevřena zastavená. Computer Use ověřil ovládací okno a náhled skutečného klienta; náhled se při přepnutí do aplikace správně označil jako pozastavený.
- Kalibrace oblasti byla provedena tažením na náhledu a uložena do lokálního `config.json`; po restartu se oblast načetla.
- Izolované prostředí: Python 3.14, OpenCV modul 4.13.0 (distribuce 4.13.0.92), NumPy 2.3.5, Pillow 12.2.0. `pip check`: No broken requirements found. Globální tři OpenCV distribuce zůstaly beze změny.
- Nezávislá kontrola nalezla čtyři Important vady; opraveny s reprodukčními testy. Navazující živé kontroly přidaly ustálení popředí, systémové klávesové události, zachování důvodu pauzy a synchronní potvrzení polohy kurzoru.

## Živé vstupy

Živé doplnění fronty, F7/F8, změna zaměření během aktivního běhu, pokračování útoku a otočení kamery nejsou potvrzené. Dlouhodobý běh nebyl proveden.

Měřený start z tlačítka prošel do `Action FILL requested`, ale pohyb kurzoru byl odmítnut: `[WinError 5] Access is denied` z `SetCursorPos`. Log `diagnostics/live-test.log` zaznamenal 12:47:09 žádost a okamžitou pauzu. Výběr skončil před Ctrl a myš-down. Předchozí měření prokázalo krátké překrytí při aktivaci okna; start nyní čeká na stabilní popředí. Samotné potvrzení událostí SendInput nebo registrace F7/F8 není důkaz viditelného chování ve hře.

Další krok: zavřít nástrojem spuštěnou instanci, otevřít `Spustit bota.cmd` přímo z uživatelovy plochy a provést jednu omezenou akci, ověřit paprsky, útok a F8. Dokud toto neproběhne, nelze tvrdit, že bot úspěšně doplňuje živou frontu. Nepřidávat obcházení ochrany nebo alternativní neověřené backendy.


## Navazující oprava startu a oprávnění

- 2026-10-11: uživatel potvrdil stejnou chybu i při ručním startu. Původní přisouzení pouze prostředí nástroje bylo nedostatečné.
- Read-only GetTokenInformation(TokenElevation): klient PID 29516 elevated=True; vlastní testovací bot PID 34860 elevated=False; diagnostický proces elevated=False. Prokázán nesoulad oprávnění. UIPI je relevantní překážka SendInput, ne důkaz veškerých příčin SetCursorPos WinError 5.
- Start nově před aktivací ověří oprávnění, aktivuje zvolené HWND přes SetForegroundWindow, ověří skutečné popředí a teprve potom odpočítává. Aktivace nekliká na herní cíl. F7 má stejnou kontrolu oprávnění před odesláním akce.
- SetCursorPos odstraněn. SendInput pohyb vyčká na měřenou polohu nejvýše 250 ms; žádný náhradní input backend. Přesné místo/počet odmítnutých událostí je součást hlášky.
- 54/54 testů PASS, compileall PASS, pip check clean. Nové testy pokrývají aktivaci před arm, odmítnutou aktivaci bez startu, opožděné doručení pohybu, nedoručený pohyb a nesoulad oprávnění.
- Computer Use při navazujícím pokusu dvakrát odmítl aktivaci vlastního okna: failed to activate captured window. Živá automatická aktivace a výběr proto zůstávají neověřené. Pouze vlastní diagnostická instance ukončena; F7/F8 uvolněny ukončením procesu.
- Zbývá ručně spustit launcher jako správce a potvrdit UAC, pak ověřit jeden výběr, útok, F8 a opakované doplňování. UAC nesmí potvrzovat agent.


## Oprava předčasného odmítnutí aktivace

Uživatel po spuštění jako správce dodal modal Aktivace hry. Ten neobsahuje WinError 5: pochází z okamžité kontroly GetForegroundWindow po SetForegroundWindow. Ta je chybná pro asynchronní přepnutí mezi procesy. API nyní jen žádá o aktivaci, vrací přijetí žádosti pro log a Worker čeká na skutečné ustálení popředí až 12 s; bez něj neodesílá vstupy. Skutečné odmítnutí OS není obcházeno.

55/55 testů PASS a compileall PASS. Nové testy: přijatá žádost bez okamžitého popředí a odmítnutá žádost bez myšího/klávesového vstupu. Živé přepnutí po této opravě není ověřeno. Nový požadavek na profily/barvu/panel je zatím specifikace docs/superpowers/specs/2026-10-11-profile-target-design.md, nikoli implementovaný režim.

Primární vysvětlení: https://devblogs.microsoft.com/oldnewthing/20161118-00/?p=94745


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

## 2026-10-12 — English application text

- All app-owned labels, tooltips, dialogs, validation errors, run states, countdown descriptions and Windows input diagnostics use English. Machine configuration keys and stored user profile/skill names are unchanged. The existing launcher filename remains Spustit bota.cmd; its setup message is English.
- State producers and consumers were updated together in controller, runtime, worker, App, overview and theme. English expectations replace former Czech assertions; user-name preservation tests retain their Czech input names.
- 222/222 unittest PASS (23.826 s); compileall PASS. Real-widget verify_theme, verify_mount_switch, verify_skill_intervals and verify_buff_icons PASS. Native 980×640 dark preview visually checked: translated labels fit, statistics remain separate, no overlaps. Existing profiles were loaded in memory without saving; no game inputs sent.
- Independent read-only review found no untranslated app-owned text or mismatched state comparisons. README UI captions match the English controls.

## 2026-10-12 — Sidebar dark-theme borders and separators

- Reproduced the bright tab edges and raised separator rules in the native compact dark fixture. Clam's TNotebook.Tab lightcolor map overrides the global palette; separator elements retain system-colored relief.
- Notebook and tab edge colors now explicitly use the current palette for all states. Removed Clam's selected-tab padding override to keep tab geometry stable. Both separator orientations use one flat palette-colored image element, cached per theme on the owning Tk root.
- verify_theme failed on the original tab lightcolor, then passed after the fix in both themes; checks edge colors across normal/selected/hover/disabled and one-pixel separator thickness. compileall PASS. Final screenshot verification was interrupted by the user's physical Escape; no further Computer Use actions taken.
- Full regression verification: 222/222 unittest PASS (24.862 s). No user profile writes or game input.

## 2026-10-12 — Native title bar follows the application theme

- Added window_chrome.apply_title_bar, called by the single apply_theme path at startup and theme changes. Resolves Tk's outer decorated HWND through GetAncestor(GA_ROOT) with pointer-safe ctypes signatures. Applies DWM dark-frame, caption, text and border colors from the same palette. Native minimize/maximize/close, drag and resize behavior remain managed by Windows.
- DWM HRESULT failures are logged without affecting the bot; no privilege elevation, game input or system preference changes. No custom title-bar replacement or duplicate theme state.
- Reference: https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute . Caption/text/border attributes are set-only on the tested Windows build (Get returns E_INVALIDARG); verify_theme reads the supported dark-frame attribute and verifies both transitions.
- 222/222 unittest PASS (25.392 s), compileall PASS, verify_theme PASS including native DWM dark flag in dark/light. Native compact screenshot visually verified the dark caption and caption buttons. The final light screenshot was interrupted by the user with physical Escape; no more Computer Use actions were taken. Native DWM verification of the light transition passed. Disposable fixtures only; user profiles unchanged.

## 2026-10-12 — Remember mount On/Off outside profiles

- UISettings.last_mounted is a validated boolean (default False for first launch or existing theme-only settings). Theme changes use dataclass replace so they preserve it. User declarations and acknowledged automatic mount transitions write preferences immediately, without saving or dirtying a server profile.
- Mount UI now has only On/Off. Pending acknowledgements protect the latest choice from old samples. New worker session identities restore the remembered value; reconfiguring the same identity preserves live state and timers. An uncertain failed mount input still pauses the controller until manually corrected; its None is never persisted or presented as a third switch state.
- Real-widget verify_mount_switch PASS: On/Off, stale acknowledgement rejected, both values persisted in disposable preferences, no profile mutation. verify_theme PASS including native title-bar transitions. compileall PASS. Existing production profiles/preferences were not written; fixtures use their own directory.
- Final regression run: 223/223 unittest PASS (27.616 s), including restore on new worker identity and preservation of the live value when configuring the same identity.

## 2026-10-12 — Compact mount state control

- Removed duplicate I am mounted caption and separate status label. Current mount state now has one native switch whose textvariable is the canonical On/Off status. No duplicate state widgets or styling remain. The no-window message uses the run reason rather than replacing the switch value.
- Disabled continues to mean no selected game window; it protects the window-scoped worker declaration. The game does not need foreground focus just to declare state.
- 39 App tests PASS, real-widget verify_mount_switch PASS (one bound On/Off caption, persistence and stale acknowledgement guards), compileall PASS. README updated; no game input or production profile writes.

## 2026-10-12 — Mount preparation before selecting a game window

- Current mount state remains enabled without a selected game window. Offline changes persist the canonical last_mounted preference and send no worker/window command. After choosing a window, the existing configure path restores it into a new session. Online declarations retain identity checks, acknowledgement and pause behavior.
- Offline UI updates ignore previously attached-window samples, so they cannot replace the new choice. No duplicate state storage or new runtime path was added.
- 224/224 unittest PASS (25.333 s), compileall PASS. Real-widget verify_mount_switch PASS: both offline directions, persistence, no worker commands, then existing online/stale acknowledgement checks. README and buff spec updated. No game inputs or production profile/preferences writes performed.

## 2026-10-12 — Dropdown arrow hover colors

- Reproduced Clam's inherited TCombobox active/pressed background #eeebe7 and unconfigured arrow color. The shared combobox style now explicitly uses palette surface/selected backgrounds and muted/foreground arrow colors, with disabled colors and palette focus border.
- Applies to all app dropdowns, including readonly buff modes and interval units. verify_theme PASS in both themes for active/pressed/readonly/disabled resolved colors; 4 theme tests PASS; compileall PASS. No game input or production data writes.

## 2026-10-12 — Numeric input stepping

- Replaced all numeric settings, mount timing, skill waits and intervals with native ttk.Spinbox. Profile/skill names and keybinds keep their existing controls. Arrows and keyboard Up/Down update draft state; calibration arrow steps redraw detection. Manual typing remains allowed, with existing save-time validation.
- NumberRule/NUMBER_RULES is now the canonical shared validation range and editor step source. Original validation limits and integer requirements preserved; no copied UI validation logic. Shared number_input uses non-wrapping ranges and explicit decimal precision to avoid Tk rounding low offsets (e.g. .0001 + .001). Arrow edits remove display-only trailing zeros.
- Interval unit changes update both conversion and spinbox bounds (10–86400 seconds or their minute equivalents); steps are one selected unit. Shared theme covers spinbox fields, arrows, hover, pressed and disabled states.
- 226/226 unittest PASS (27.029 s), compileall PASS; new real-widget tests verify native increments/decrements, integer and decimal steps, boundary clamps, change notification, and interval duration after unit changes. verify_theme PASS in both palettes including spinbox hover/pressed/disabled colors. Native compact dark Settings screenshot checked: all numeric arrows visible and fields fit sidebar. No game inputs or production profile writes.

## 2026-10-12 — Sidebar spacing and tab focus

- Tk's notebook ActivateTab automatically traverses to the first page control unless the notebook already owns focus. Its widget click binding now focuses the tab strip before the native class binding; native tab selection/keyboard navigation remain in place and page inputs are no longer selected by a tab click.
- Sidebar widened from 238 to 280 px. Settings grid gives spare width to its label column and keeps numeric fields right-aligned inside the existing symmetric 10 px content padding, rather than clipping the grid at the scrollbar.
- verify_theme PASS on the real App: clicked all tabs, notebook kept focus; visible settings inputs retain at least 10 px on each outer edge. Full 226/226 unittest PASS (27.842 s), compileall PASS. Native 980×640 dark screenshot checked wider sidebar, visible numeric arrows, right gutter and unselected profile input. No game input or production profile writes.

## 2026-10-12 — Documentation, repository hygiene and UAC launcher

- Rewrote README as a coherent Czech setup/usage/development guide with current English UI captions: Windows/Python/Tk requirements, pinned dependencies, installation, launcher, source-run/no-EXE build status, tests, calibration, buff/toggle/mount behavior, timers/statistics, data backup and troubleshooting. Removed stale confirmation-button, unknown-switch and old launcher instructions.
- Renamed the launcher to run.cmd. It delegates to tools/launch.ps1, checks local virtual-environment interpreters and imports, then uses one Start-Process -Verb RunAs invocation for the visible Pythonw application with an explicit project working directory. CheckOnly validates dependencies without GUI/UAC. Cancelled UAC exits 2, other failures 1; CMD retains error output. No system UAC or persistent execution-policy changes. Live UAC was not triggered or confirmed during verification.
- Windows privilege error now points to run.cmd. Updated .gitignore for environments, caches, private profiles/preferences, diagnostics artifacts and build output; diagnostic Python sources and test images remain visible.
- Launcher tests mock only Start-Process, exercising actual Windows PowerShell dependency checks, one RunAs request, interpreter/arguments/working directory, cancellation and missing environment. Git ignore behavior verified in an isolated temporary repository, without initializing the project repository. Read-only CheckOnly PASS, pip check PASS and compileall PASS.
- Final full regression run: 230/230 unittest PASS (29.570 s). Production profiles and application preferences were not modified.

## 2026-10-12 — Explicit local profile ignore coverage and distribution feasibility

- Local-data ignore section now explicitly covers the root profiles directory (all JSON, selection, template PNGs and nested temporary files), root UI preferences, legacy config/migration backup and any interrupted root atomic JSON writes. Shared-source examples and test fixtures remain visible.
- Real git check-ignore test PASS in an isolated repository, expanded to profile PNGs, selection, all actual atomic .json.tmp paths, legacy config and backup. No production data deleted or rewritten.
- README documents future bundled-app requirements, clearly marked not implemented: self-contained Windows installer with a directory bundle, stable user-data storage/migration and clean-machine verification. Official PyInstaller operating-mode and runtime-information docs checked; no new dependency or packaging implementation installed.

## 2026-10-12 — English README

- Translated the entire README to English, including setup, pinned requirements, UAC launch behavior, source build status, calibration, editor controls, statistics, buff detection, mount/timers, local data, troubleshooting and future distribution notes. Commands, paths, limits and verified/unverified distinctions preserved.
- Checked local Markdown links, balanced code fences, retained setup commands and absence of Czech authored text. Documentation-only change; runtime tests were not rerun because no executable code changed.
- Reviewed official GitHub Actions and PyInstaller documentation for the proposed next phase. Automated packaging/release workflow is not implemented or published by this change.

## 2026-10-12 — Windows distribution implementation

- Implemented one shared source/frozen CLI, PyInstaller onedir x64 application with native administrator manifest, and pinned Inno Setup 6.7.3 installer. Runtime/build dependencies are pinned; VERSION is the stable version source. Build scripts validate metadata, compiler and dependency versions, keep generated files under build/dist, and produce an installer plus SHA256SUMS.txt.
- All writable profiles, templates, selection, preferences and logs use LocalAppData/MetinBot. Atomic staged import preserves originals and rejects invalid data, conflicting legacy files and overwriting a nonempty destination. The first installed launch offers explicit import; Cancel creates no new data. Diagnostics and self-tests use injected disposable storage. Production profiles and preferences were not changed during verification.
- Windows Actions workflow validates PRs without publication credentials, builds unique main prereleases, and publishes stable tags matching VERSION on main. Pinned actions, separate contents-write release job, checksum/commit/tag checks and draft publication/resumption are configured. New prereleases explicitly create the remote tag before staging a draft; published releases remain immutable. Mock tests cover new, resumed, published and conflicting releases.
- Full regression: **256 tests PASS (43.207 s)**. pip check, compileall, PowerShell syntax and actionlint PASS. Independent review found and verified fixes for first-launch legacy import and draft/tag lifecycle.
- Actual final directory bundle verification PASS: x64 architecture, embedded version/commit, dependencies, no user data and requireAdministrator manifest. Actual frozen EXE isolated self-test PASS: Tk/Pillow in both themes, synthetic OpenCV/NumPy detection and profile/preference roundtrip. Standard RunAs launch was used; no UAC confirmation or game input was automated.
- Final installer build PASS: dist/release/MetinBotSetup-0.1.0-dev.1-windows-x64.exe. SHA-256: `9a21b8f04ca5d2e43cd8ecd2b592d6addc5400b3cebc1406c55d255d022cf21e`. Local build commit is the explicit unversioned marker 0000000.
- Limits: this folder has no Git repository/remote, so no push, remote workflow or GitHub release was performed. Actual installer installation/uninstallation is configured for a disposable hosted Windows runner and was not run on this user's machine. Operation on a clean Windows machine without Python and real-client compatibility remain unverified. Binaries are unsigned.

## 2026-10-12 — First GitHub push and CI acceptance

- Initialized the source checkout against the existing main history and pushed commit 28fd08f to ethanjojo/metin2-bot. Existing remote history was preserved. The index excludes local profiles/preferences, logs, environment and build artifacts. Fresh local regression: 256 tests PASS (34.441 s); compileall, pip check, actionlint and staged whitespace checks PASS.
- GitHub run 38307564847 successfully verified source, compiled the installer, installed it in its disposable Windows runner, executed the installed bundled self-test, uninstalled it and uploaded artifacts. This verifies actual CI installation, not gameplay or interactive UAC on a user's computer.
- Publication exposed an API contract error: GET commits/<missing-tag> returns HTTP 422, whereas the helper expected a missing-ref 404. The tag check now first uses GET git/ref/tags/<tag>, then resolves existing lightweight/annotated tags to the tested commit. The new-release mock reproduces the real 422 response: failed before the fix and passed after it; all four release tests PASS. A new push revalidates the complete workflow.
