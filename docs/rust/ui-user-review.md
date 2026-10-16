> Historical verification report. Use the repository README and docs/release-readiness.md for current installation/release status. Earlier commands, counts and package paths below describe their original snapshots.

# Uživatelská kontrola Rust UI — 2026-10-16

Kontrola našla sedm konkrétních problémů; všechny byly opraveny a při závěrečné kontrole nezůstal další potvrzený blokující nález. Opravy sjednocují klávesové a myší akce, zpřehledňují spuštění a zakázané ovládání a odstraňují dvě chyby kalibrace. Nešlo o ruční ovládání nativní aplikace: tento běh nemá dostupné nativní CUA. Interakční důkazy vznikly skutečnými událostmi egui `RawInput` nad aplikací; vizuální důkazy pocházejí z PNG skutečně vykreslené aplikace.

## Nálezy a jejich opravy

| Priorita | Reprodukce a dopad | Oprava a důkaz |
| --- | --- | --- |
| P2 | Na kalibračním canvasu Delete, Ctrl+Z a Ctrl+Y měnily profil bez volání zastavení runtime, zatímco odpovídající tlačítka zastavení volala. Akce měly rozdílnou semantiku a klávesová editace nerušila čekající spuštění. | Klávesnice i tlačítka používají společné `delete_selected`, `undo_profile`, `redo_profile`, které zastaví runtime a zruší gesto. RawInput test ověřuje Delete → Undo → Redo nad stejnou oblastí; shodu zastavení ověřuje kontrola společné produkční cesty. [state.rs](../../crates/metin-desktop/src/ui/state.rs), [mod.rs](../../crates/metin-desktop/src/ui/mod.rs). |
| P2 | Standardní `cargo test -p metin-desktop --locked --offline` nekompiloval: test porovnával `Tab` přes `assert_eq!`, ale `Debug` existoval jen s feature `ui-smoke`. | `Tab` odvozuje `Debug` vždy. Původní compiler výstup měl E0277 na `tests.rs:89`; po opravě prošel výchozí lib target. [mod.rs](../../crates/metin-desktop/src/ui/mod.rs). |
| P2 | Po požadavku Start byl runtime ve stavu Armed, ale `running` ještě zůstával false. Start dál nabízel nové spuštění a opakovaný klik znovu konfiguroval profil a restartoval odpočet. Run mohl současně hlásit Ready to start. | Snapshot nese explicitní `arming`; toolbar ukazuje zakázané Starting..., Run vysvětluje čekání a Pause/F8 zůstává dostupné. Syntetický publikovaný stav je vykreslen přímo přes toolbar a Run bez připojení hry. [runtime.rs](../../crates/metin-desktop/src/runtime.rs), [shell.rs](../../crates/metin-desktop/src/ui/shell.rs), [run.rs](../../crates/metin-desktop/src/ui/run.rs). |
| P3 | Tooltip Save & Start naznačoval, že F7 má stejný účinek. F7 ale spouští naposledy aplikovaný profil a neukládá rozepsané změny; vysvětlení bylo jen na stránce Run. | Tooltip rozepsaného profilu nyní výslovně odlišuje uložení editací tlačítkem a spuštění aplikovaného profilu F7. [shell.rs](../../crates/metin-desktop/src/ui/shell.rs). |
| P3 | Zakázaný Snapshot měl obecný návod k zachycení, ale nevysvětloval chybějící vybrané okno ani již probíhající zachycení. | Tooltip rozlišuje chybějící okno, čekání na snímek s limitem tří sekund a dostupnou akci. RawInput průchod pending → timeout → Dismiss ověřuje návrat tlačítka Snapshot a zachování předchozího snímku. [shell.rs](../../crates/metin-desktop/src/ui/shell.rs), [tests.rs](../../crates/metin-desktop/src/ui/tests.rs). |
| P3 | Vnořené Target panel a Reference crop se sdíleným levým horním rohem kreslily popisky přesně na sebe. Text na canvasu byl nečitelný; projev byl vidět v obou tématech. | Canvas kreslí jediný popisek právě hoverované oblasti, jinak vybrané oblasti. Regresní test před opravou selhal na překryvu stejných obdélníků textu; po opravě kontroluje i skutečný hover checklistu bez změny výběru. [preview.rs](../../crates/metin-desktop/src/ui/preview.rs). |
| P3 | Krok Metin color byl Ready, ale Delete selected zůstával zakázaný. Stejnou hodnotu přitom šlo smazat klávesou Delete. | Viditelné tlačítko umí smazat také vybranou barvu a vysvětluje stav, kdy není co smazat. Test před opravou reprodukoval nezměněnou barvu; po opravě klik smaže a Ctrl+Z obnoví původní hodnotu. [calibration.rs](../../crates/metin-desktop/src/ui/calibration.rs). |

## Projité uživatelské kroky

1. **První otevření a prázdný profil — srozumitelné.** Run uvádí blokující podmínku a další akci. Buffs nabízí první skill; Settings vysvětlí, proč ještě nelze upravit barevné tolerance. Nový neuložený profil je označen jako Unsaved changes.
2. **Navigace a profily — ověřeno egui událostmi.** Přechody mezi Run, Calibration, Buffs a Settings zachovávají rozepsané změny. Klik přes skutečný profilový ComboBox přepne čistý uložený profil, uloží selection a odstraní předchozí canvas a návratovou cestu ke skillu. Disabled Save i Start zobrazují při skutečném hoveru nápovědu.
3. **Kalibrace — ověřeno egui událostmi a obrazem.** Hover všech čtyř kroků zvýrazní odpovídající oblast bez změny výběru, klik změní režim. Hide/Show steps zachová výběr i profil a rozšíří canvas. Existující průchody pokrývají tažení, Escape a Undo/Redo; nové pokrývají mazání barvy a kolize popisků. Canvas má instrukce pro zoom a pan.
4. **Snapshot a chyba — ověřeno egui událostmi.** Capturing... přejde po vypršení limitu zpět na Snapshot. Chyba nabízí Dismiss a doporučuje kontrolu okna. Zavření hlášky nezničí předchozí editovatelný snímek.
5. **Buffy a mount — ověřeno egui událostmi.** Escape ukončí záznam klávesy, F7 zůstane rezervované, Ctrl+A se zaznamená, Backspace vymaže zkratku bez smazání řádku. Přepnutí sekund na minuty zachová délku intervalu a přepínání režimu mění jen vybraný skill. Existující testy pokrývají reorder/remove, osm skillů, scroll k icon capture a návrat ke stejnému skillu. Nový test skutečně klikne Confirm selected state při neznámém mount stavu a ověří okamžitou deklaraci bez změny profilu na dirty.
6. **Start a Pause — ověřen vykreslený syntetický stav.** Armed zobrazuje čekání a Starting..., nikoli Ready to start. Pause/F8 zůstává viditelné. Skutečné startování hry nebylo součástí kontroly.
7. **Nastavení a různé velikosti — bez nového blokujícího nálezu.** Prohlédnuty všechny čtyři stránky v dark/light při 800×560 a 1280×820, prázdné stavy a Linux snímky. V malé velikosti je nutné scrollovat postranní seznam i detail; hlavní navigace a Pause zůstávají dostupné. Číselné vstupy používají sdílené rozsahy domény, nápovědy vysvětlují jednotky a vliv hodnot.

## Ověření a hranice

- Přibylo 11 pojmenovaných testů `user_review_*` v [ui/tests.rs](../../crates/metin-desktop/src/ui/tests.rs). První čtyři byly samostatně spuštěny s `--features ui-smoke --locked --offline`; všechny prošly. Následný koordinovaný výchozí lib běh po opravách hlásil 31/31. Poslední dva testy nejprve reprodukovaly chyby a po opravách prošly.
- Kontrola změněných produkčních cest a `git diff --check` proběhla nezávisle po opravách. Koordinovaná závěrečná Windows verifikace hlavního běhu hlásila 89/89 testů celého workspace, výchozí lib target 31/31 a úspěšný formát, strict Clippy i build.
- Nativní dialogy Save/Discard/Cancel při přepnutí rozepsaného profilu nebo zavření aplikace nebyly ručně odklikány. RawInput se nedostane do systémového dialogu `rfd`; čisté přepnutí profilu bylo odklikáno v egui.
- Reálné vyhledání herního okna, zachycení jeho obsahu, odpočet se skutečným focusem, globální F7/F8 a odeslání vstupů do hry nebyly zkoušeny. Server byl offline a kontrola nepřipojovala živý game target ani neposílala Configure/Arm pro hru.
- Linux obrázky dokazují vykreslení a stav nedostupného backendu, nikoli funkční capture/input. Simulované HiDPI a RawInput neprokazují chování při fyzickém přesunu okna mezi monitory ani úplnou přístupnost pro asistivní technologie.
- PNG fixtures používají syntetický herní obraz. Neprokazují čitelnost detekčních overlayů nad každou skutečnou herní scénou.

## Obrazové podklady

Snímky jsou lokální testovací artefakty pod `build/ui-redesign/`; nejsou součástí distribuované aplikace. Opravované místo je nejlépe patrné na následujících čtyřech kalibračních snímcích:

- [Dark, 1280×820](../../build/ui-redesign/dark-calibration-large.png)
- [Dark, 800×560](../../build/ui-redesign/dark-calibration-small.png)
- [Light, 1280×820](../../build/ui-redesign/light-calibration-large.png)
- [Light, 800×560](../../build/ui-redesign/light-calibration-small.png)

Po finálním debug buildu byly všechny čtyři soubory znovu zachyceny a nezávisle otevřeny. V obou tématech a velikostech je na canvasu jediný čitelný popisek Search area; nevybrané vnořené oblasti zůstávají ohraničené bez kolidujících popisků. Navigace, Hide/Show, zoom a Pause zůstávají v okně. Hover variantu, kterou statický snímek neukazuje, ověřuje výše uvedený RawInput regresní test.
