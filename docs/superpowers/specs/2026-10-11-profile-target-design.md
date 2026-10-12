# Profily serverů a postupné klikání na metiny

Stav: návrh k posouzení, zatím neimplementováno. Nahrazuje frontový způsob řízení po schválení této změny; nevytváří druhý souběžný runtime.

## Zadání

Uživatel chce vybrat herní okno, vytvořit profil konkrétního serveru, vybrat barvu metinu pickerem a v náhledu vidět všechny nalezené kandidáty. Dále označí stálou část panelu cíle, například křížek spolu s dalším prvkem panelu. Výslovně zvolil klikání na jednotlivé metiny a čekání na zmizení panelu namísto doplňování fronty Ctrl + tažením.

Úspěch: po kalibraci bot sám vybere kandidáta, jednou klikne, potvrdí panel cíle, během jeho přítomnosti nepřeklikává na jiné kameny a teprve po stabilním zmizení pokračuje. Před delším během musí projít živé ověření kliknutí, příchodu k cíli, panelu, pokračování a F8.

## Zvolená detekce a její omezení

Použít HSV barvu a filtry tvaru/velikosti. Uživatel klikne na tělo metinu v pozastaveném náhledu; median malého okolí bodu určí barvu. Tolerance barvy a minimální/maximální plocha jsou explicitní nastavení. Hue se porovnává kruhově. Detekce probíhá jen v označené oblasti světa, mimo UI. Profil dovolí několik obdélníkových masek vyloučení pro panel cíle, minimapu, inventář a lišty. V maskách se nehledají kandidáti ani body kliknutí; panel se vyhodnocuje samostatně ve své oblasti. Spojené barevné oblasti jsou kandidáti, nikoli prokázané metiny. Náhled zobrazí jejich rámečky a body kliknutí. Při více kandidátech se použije stabilní pořadí podle vzdálenosti od středu herní oblasti, nikoli tvrzení o skutečné vzdálenosti ve světě.

Oproti porovnávání celého obrázku metinu je barva méně citlivá na natočení, ale může zaměnit podobně barevného nepřítele nebo terén. Naučený detektor může pomoci později, vyžadoval by označené příklady z konkrétních serverů. První verze proto nabídne dobře kontrolovatelnou barevnou detekci a její kalibraci, bez dalšího detekčního backendu.

Panel cíle: uživatel označí oblast panelu a uvnitř ní stálý, dostatečně výrazný výřez. Výřez se uloží jako místní PNG a porovnává jen v oblasti panelu normalizovaným template matchingem. Práh a počet potvrzujících snímků budou v profilu. Odmítnout velmi malý nebo téměř jednobarevný výřez. Samotný univerzální křížek může odpovídat jiným oknům; rozšíření výřezu o další stálé prvky a omezení oblasti hledání snižuje tuto záměnu. Dodaný malý obrázek slouží jako příklad, nikoli automaticky přijatá kalibrace pro živý Verzum.

Zmizení panelu znamená **ukončení cílení**. Může jej způsobit zničení, odznačení, vzdálení nebo změna UI. Aplikace nebude hlásit přesný počet zničených metinů pouze z tohoto signálu. Při nejasném nebo neplatném snímku se běh pozastaví, nepovažuje se za zmizení.

## Jeden řídicí tok

1. Zastaveno: pouze náhled a kalibrace. Profil musí mít okno, oblast, barvu a šablonu panelu.
2. Aktivace: jednou požádat Windows o vybrané okno, čekat na skutečné ustálené popředí; žádný vstup bez potvrzení. Nesoulad oprávnění odmítnout s konkrétní instrukcí. UAC potvrzuje uživatel.
3. Hledání: stabilní kandidát ve více snímcích. Pokud panel cíle již existuje při startu, nejprve čekat na jeho zmizení, aby se nepřerušoval aktuální útok.
4. Kliknutí: jeden běžný levý klik do ověřeného kandidáta, bez Ctrl. Před stiskem znovu zkontrolovat okno, geometrii, překrytí, zrušení a pozici kurzoru.
5. Čekání na panel: po kliku musí být panel potvrzen v několika snímcích. Dokud se nepotvrdí, jeho nepřítomnost není ukončení cíle. Timeout 20 s je počáteční nastavitelná hodnota, nikoli naměřené trvání příchodu.
6. Cíl aktivní: žádné nové klikání ani otáčení. Potvrzená nepřítomnost panelu ve třech snímcích umožní další hledání. Útok má nezávislý limit 120 s; po jeho překročení pauza s důvodem, ne odhad zničení.
7. Nenalezený kandidát nebo nepotvrzený klik: krátké otočení přes E, ustálení a nové hledání; po čtyřech neúspěšných pokusech pauza. Tyto limity jsou viditelné a nastavitelné. Kandidáta po neúspěšném kliku krátce vyřadit jen v aktuálním pohledu; po otočení se evidence pohledu ruší.

Stejný Worker, Runtime a guarded InputDriver obsluhují všechny profily. F8, zavření a ztráta zaměření ukončí vstupy. WASD, léčení, smrt, login a čtení paměti hry zůstávají mimo rozsah.

## Profily a UI

Každý profil má UUID, název serveru, název procesu, oblast světa, masky vyloučení, nastavení barvy a geometrie kandidátů, oblast panelu, šablonu a časové limity. Vybrané živé HWND/PID je dočasný stav, ne ukládaná identita. Při více odpovídajících oknech vyžadovat výběr.

Jediná konfigurace serveru bude `profiles/<uuid>/profile.json`, se související místní šablonou panelu. Poslední vybraný profil je samostatná preference aplikace, bez kopie kalibračních dat. Zapisovat šablonu do nového souboru a JSON atomicky až po jejím úspěšném zápisu; odstranit nahrazená nepotřebná aktiva. UUID nepřijímá libovolnou cestu. Neznámá pole, neplatné prahy, chybějící aktiva nebo rozdílná kalibrační geometrie start odmítnou.

Tok nastavení: **profil → živé okno → oblast světa → picker barvy → náhled kandidátů → panel a stálý výřez → uložit → start**. Barevný picker a výřez se vybírají na pozastaveném snímku; tlačítko Test detekce neodesílá herní vstupy. Změna profilu nebo kalibrace pozastaví běh před použitím nové konfigurace. Nastavení jiného serveru nesmí tiše použít šablonu Verzumu.

## Čistá migrace

Převést použitelný název procesu a oblast ze současného config.json do úvodního profilu Verzum. Nový profil začíná zastavený a nekompletní, dokud uživatel nenakalibruje barvu a panel. Po úspěšném převodu odstranit původní config.json jako aktivní zdroj, před převodem vytvořit označenou zálohu. Odstranit paprskový detektor, Ctrl-drag akci, frontové stavy, staré prahy a nepoužívané testy/dokumentaci; žádný přepínač na starý režim. Zachovat nezávislé kontroly Windows, geometrie, oprávnění, zrušení a lifecycle.

## Ověření

- Profily: izolace kalibrací, validace, atomický zápis, jednoznačná migrace, odmítnutí chybějící šablony.
- Obraz: kruhový odstín, tolerance, vyloučené UI, filtrování kandidátů, více kandidátů; panel přítomen/nepřítomen, podobné jiné okno a málo výrazná šablona.
- Řadič: klik jednou, chybějící první panel není úspěch, potvrzený panel brání překlikávání, debounce zmizení, timeout nezávislý na animaci, počáteční aktivní panel, omezené hledání.
- Vstupy: bezpečný levý klik, zrušení během stisku a otočení, nedoručený pohyb bez stisku, ztráta zaměření, čekání na asynchronní aktivaci.
- UI: vytvořit dva profily, nakalibrovat jeden a přepnout na druhý bez sdílení dat; náhled musí odpovídat skutečné detekci.
- Živě na Verzumu: správná volba cíle, panel při útoku, zmizení a další výběr, F8, ztráta popředí. Rozlišit automatické testy od skutečně pozorovaného chování.

## Otevřená kalibrace

Poloha a konkrétní stálý výřez skutečného panelu Verzumu se určí v kalibračním UI. Před jeho výběrem ani snímek křížku, ani žlutý paprsek nejsou dostatečným důkazem správné detekce panelu. Spolehlivost barvy se posoudí na živém náhledu před spuštěním vstupů.


## Důkazy z dodaného videa BeaTzTools

Analyzován místní soubor `C:/Users/horak/Downloads/ssstik.io_@beatztools_1791717697899.mp4`, 576×1024, 864 snímků při 30 FPS, délka 28,8 s. Odečty níže jsou pozorování obrazových snímků; interní algoritmus ani použitou knihovnu video neodhaluje.

- 6–8 s: Server / Window Name, pole Origins a tlačítko Pick. UI zmiňuje výběr existujícího serveru nebo vytvoření nového.
- 9 s: Metin Detection, Profile Default, New, Pick Color, Live Color Detection, Draw Mask, Remove Masks. Profily a masky jsou skutečně přítomné.
- 10–11 s: obdélníkový výběr stálé části panelu kolem ikon W / otazník / křížek. Instrukce výslovně doporučuje stálou část a nikoli HP bar. To podporuje šablonu stálého výřezu, ne porovnávání měnícího se panelu jako celku.
- 12 s: zelené rámečky jsou i na herních ikonách a portrétu. Po změně vybrané barvy kamene ve 14 s ukazuje snímek 15 s rámečky na kamenech. Samotná barva proto musí mít živý kontrolní náhled a vyloučené UI.
- 16 s: Combat & Timing nabízí multi-client, loot key, unstuck skill, queue timeout, pre-click delay a rychlost. 18 s ukazuje Buffs & Extras. Nejde o automatické rozšíření našeho schváleného rozsahu.
- 20 s: tlačítka Start (F5), Stop (F6), Bio Mode, Macro Mode a stav QUEUE RUNNING. Nelze tvrdit, že reklamní ukázka předvádí výhradně jednotlivé klikání bez fronty.
- 22–24 s: útok, panel cíle a počítadlo Destroyed Metins 0 → 1. Zobrazený čas současně přeskočí 00:00:09 → 00:00:18; střih není důkaz spolehlivosti detekce zničení ani kompletního přechodu mezi cíli.

Náš zvolený tok jednotlivých kliknutí zůstává podle výslovné volby uživatele. Z videa přebíráme ověřitelné požadavky na kalibraci: profily, picker s náhledem, stálý výřez panelu a masky. Nepřebíráme tvrzení o spolehlivosti farmení ani rozšiřující funkce bez dalšího rozhodnutí.

## English interface

The application has a single English UI, including dialogs, tooltips, validation and runtime diagnostics. User-defined profile and skill names are preserved verbatim; persisted keys and machine mode identifiers remain stable. Labels used as runtime state identifiers are translated consistently across their producers and consumers.
