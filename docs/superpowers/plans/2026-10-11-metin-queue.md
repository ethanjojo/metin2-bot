> SUPERSEDED: historical queue-mode record. Active design: 2026-10-11-profile-target-design.md; runtime now uses individual clicks and target panels.

# Metin Queue Bot Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Lokální Windows aplikace doplňující frontu metinů ve Verzum pomocí Ctrl + levého tažení, s náhledem a nouzovým zastavením.

**Architecture:** Jedna řídicí smyčka načítá snímky klientské plochy, vyhodnocuje paprsky a vydává nejvýše jednu akci. Čistý řadič a detektor jsou odděleny od Windows ovládání a Tkinter rozhraní. Fronta hry je autoritou; aplikace neudržuje vlastní seznam domnělých členů fronty.

**Tech Stack:** Python 3.14 dostupný na tomto počítači; Tkinter, Pillow ImageGrab, NumPy, OpenCV; ctypes pro Win32; unittest pro testy. Jediná cesta snímání je obraz viditelné klientské plochy přes Pillow, bez alternativních backendů. Závislosti připnout podle skutečně ověřeného místního prostředí při implementaci.

**Spec:** `docs/superpowers/specs/2026-10-11-metin-queue-design.md`

## Global Constraints

- Výchozí stav je zastaveno.
- Nepřítomnost paprsků proto nebude znamenat prázdnou frontu.
- Vstupy se odesílají pouze do vybraného viditelného okna v popředí.
- Bot nebude automaticky přebírat zaměření jinému programu.
- Každá akce uvolní Ctrl, tlačítko myši i klávesu otočení při dokončení, zastavení nebo chybě.
- Snímky zůstávají lokálně; ukládání diagnostiky je explicitní volba.
- První verze neobsahuje samostatné hledání cesty přes WASD.
- Nečte paměť hry ani neposílá vlastní herní pakety.
- Nezakládat Git ani nevytvářet commity automaticky: složka nyní není repozitář. Kroky níže uzavírat ověřením a označením v tomto plánu.

## Review Focus

- Změna DPI, velikosti nebo polohy okna: souřadnice nesmějí mířit na jinou aplikaci (úloha 3).
- Žluté poškození a světlý sníh: nesmějí se vydávat za potvrzený paprsek (úloha 2).
- Trvale opakovaná animace: změna pixelů nesmí resetovat časový limit pro potvrzení označení (úloha 1).
- Uživatel drží Ctrl nebo levé tlačítko: nevstupovat do jeho rozpracované akce (úloha 3).
- F8 během drag nebo při zavření rozhraní: akci přerušit a uvolnit pouze vstupy držené botem (úlohy 3 a 4).

## Soubory a odpovědnosti

- `metin_bot/config.py`: validované datové třídy, JSON konfigurace, normalizovaný výběrový obdélník.
- `metin_bot/controller.py`: čistý časový řadič a typy Observation, Action, Status.
- `metin_bot/vision.py`: detekce paprsků a stabilita pozorování.
- `metin_bot/windows.py`: identifikace procesu a okna, geometrie, snímání, vstupy a F8.
- `metin_bot/app.py`, `metin_bot/__main__.py`: Tkinter náhled, konfigurace a obsluha jedné smyčky.
- `tests/test_config.py`, `tests/test_controller.py`, `tests/test_vision.py`, `tests/test_windows.py`: významové testy logiky a izolovaných Windows závislostí.
- `requirements.txt`, `README.md`, `config.example.json`, `.gitignore`: reprodukovatelné spuštění a popis limitů.
- `tools/check_capture.py`: explicitně vyžádaná místní diagnostika bez odesílání vstupů, žádný druhý běh bota.

## Úloha 1: Konfigurace a řadič

**Interfaces:** `BotConfig.load(path: Path) -> BotConfig`; `BotConfig.save(path: Path) -> None`; `Controller.step(now: float, observation: Observation) -> Action | None`; `Controller.pause(reason: str) -> None`; `Controller.start(now: float) -> None`. `Observation` obsahuje `ready: bool`, `stable_beams: int`, `reason: str`; `Action` enum FILL/ROTATE; `Status` obsahuje stav a lidsky čitelný důvod.

- [x] Napsat testy `test_invalid_region_and_intervals`, `test_initially_stopped`, `test_fill_rate_limit`, `test_missing_beams_are_uncertain`, `test_confirmation_timeout_not_reset_by_animation`, `test_retry_limit_pauses`, `test_pause_prevents_actions`. Neplatné obdélníky a NaN odmítnout; interval mezi akcemi dodržet; při neplatném pozorování nedělat akci; po vyčerpání pokusů pozastavit.
- [x] Spustit `python -m unittest discover -s tests -p "test_config.py" -v` a totéž pro `test_controller.py`; ověřit selhání před implementací.
- [x] Implementovat typy a JSON validaci. Počáteční interval doplnění 8 s, čekání po výběru 2 s, limit chybějícího potvrzení 20 s, maximum otočení 4, délka otočení 0,35 s. Jde o explicitní výchozí hodnoty k živému kalibrování, nikoli naměřené hodnoty. Přetrvávající stabilní paprsky dovolují periodické doplnění; jejich dlouhé chybění spustí omezené hledání.
- [x] Spustit oba testovací soubory; očekávat PASS. Zaznamenat dokončení.

## Úloha 2: Detektor a náhledové podklady

**Interfaces:** `detect_beams(frame_bgr: np.ndarray, config: VisionConfig) -> list[Beam]`; `Beam` obsahuje `x`, `y`, `width`, `height`; `BeamTracker.update(beams: list[Beam], now: float) -> int`; `annotate(frame_bgr: np.ndarray, beams: list[Beam], region: Region) -> np.ndarray`.

- [x] Napsat `test_vertical_yellow_beam`, `test_yellow_damage_rejected`, `test_snow_rejected`, `test_partial_beam`, `test_temporal_confirmation`, `test_empty_frame_rejected`. Syntetický vysoký žlutý pás projde; krátké číslice a bílá plocha ne; jediný přechodný snímek nepotvrdí stabilní označení.
- [x] Spustit `python -m unittest discover -s tests -p "test_vision.py" -v`; ověřit selhání.
- [x] Implementovat HSV masku, spojení svislých segmentů a filtr geometrie; prostorově spojit blízké segmenty jednoho paprsku. Parametry v konfiguraci: HSV rozsah, minimální výška vůči snímku, poměr výška/šířka, počet potvrzujících snímků. Výchozí hodnoty kalibrovat na uživatelově snímku s paprsky; kontrolní snímek s modrým výběrem nesmí potvrzovat žlutý paprsek.
- [x] Spustit testy; vytvořit lokální anotovaný náhled z dodaných snímků a vizuálně zkontrolovat. Obrázky nepřidávat do projektu automaticky; diagnostické výstupy ignorovat v `.gitignore`.

## Úloha 3: Bezpečný adaptér Windows

**Interfaces:** `list_game_windows(process_name: str) -> list[GameWindow]`; `inspect_window(window: GameWindow) -> WindowGeometry`; `capture(window: GameWindow) -> np.ndarray`; `InputDriver.fill(window: GameWindow, region: Region, cancelled: Callable[[], bool]) -> None`; `InputDriver.rotate(window: GameWindow, seconds: float, cancelled: Callable[[], bool]) -> None`; `InputDriver.release_owned() -> None`; `emergency_pressed() -> bool` pro F8. `WindowGeometry` obsahuje fyzickou klientskou oblast, viditelnost a stav popředí.

- [x] Napsat testy s nahrazeným Win32 adaptérem: `test_multiple_windows_require_selection`, `test_focus_loss_blocks_input`, `test_geometry_change_interrupts_drag`, `test_user_held_input_blocks_action`, `test_cancel_releases_owned_inputs`, `test_sendinput_failure_releases_inputs`, `test_coordinate_conversion` včetně záporných souřadnic druhého monitoru.
- [x] Spustit `python -m unittest discover -s tests -p "test_windows.py" -v`; ověřit selhání.
- [x] Implementovat ctypes signatury včetně 64bitových handle/pointer typů, per-monitor DPI režim před inicializací GUI, vyhledání oken přes EnumWindows a cestu procesu. Snímat přes `ImageGrab.grab(bbox=..., all_screens=True)` a převádět RGB na BGR.
- [x] Implementovat SendInput se scan codes pro Ctrl/Q/E. Běžný pohyb kurzoru poslat přes SendInput a čekat na potvrzenou polohu nejvýše 250 ms; stisk myši poslat atomicky s absolutní pozicí po virtuální ploše. Kontrolovat počet úspěšně odeslaných událostí; před každým krokem drag kontrolovat F8, popředí a geometrii. Použít `try/finally` pro uvolnění vlastních vstupů; pokud uživatel vstupy drží před akcí, pozastavit. Selhání kvůli oprávnění zobrazit jako chybu, automaticky nezvyšovat oprávnění.
- [x] Spustit testy a bezvstupovou kontrolu živého okna pomocí `tools/check_capture.py`; porovnat snímek s obrazem klienta. Při černém obrazu nebo překrytí neprohlašovat snímání za ověřené.

## Úloha 4: Aplikace a ověření celého toku

**Interfaces:** `run() -> None` v `app.py`; `python -m metin_bot` je jediný běžný vstupní bod. Aplikace využívá rozhraní předchozích úloh, neimplementuje samostatnou řídicí logiku.

- [x] Přidat integrační test `test_stop_during_action` a `test_invalid_capture_pauses` s fake adaptérem: zastavení zabrání další akci a vždy vyvolá uvolnění; neplatný snímek se nesmí vyhodnotit jako prázdná fronta.
- [x] Spustit relevantní testy a ověřit selhání.
- [x] Implementovat jednoduché Tkinter okno: výběr klienta, náhled, výběr obdélníku tažením na náhledu, intervaly a prahy, stav a tlačítka uložit/start/pauza. Náhled je nezaměřující a nepřekrývá vybranou herní oblast; tlačítko Start jednorázově aktivuje zvolené herní okno po kontrole oprávnění; během běhu se zaměření nevynucuje. F7 umožní start/pauzu po ručním návratu do hry, F8 zastavuje. Detekovat hranu stisku F7, aby držení klávesy nepřepínalo stav opakovaně.
- [x] Obsluhovat akce v jednom pracovním vlákně, GUI jen v hlavním vlákně, výsledky přes frontu o maximální délce 1. Žádné souběžné akce; při zavření vyžádat zrušení a uvolnění. Přidat test hrany F7 a zavření během drag.
- [x] Doplnit README a příklad konfigurace: instalace závislostí, spuštění, ruční kalibrace oblasti, klávesy, omezení popředím, žádný automatický WASD. Připnout ověřené verze knihoven.
- [x] Spustit `python -m unittest discover -s tests -v`, `python -m compileall -q metin_bot tools tests` a kontrolu závislostí. Otevřít aplikaci a ověřit náhled a uložení/načtení konfigurace.
- [ ] Ve hře provést jednu omezenou akci doplnění, pořídit nový snímek, ověřit paprsky a nepřerušený útok. Poté omezený běh pro opakované doplnění a otočení; kontrola F8 a ztráty popředí. Pro vlastní ruční interakci v této chatové relaci používat Computer Use API dle skillu, nikoli ad hoc shellové UI ovládání.
- [x] Zaznamenat reálné výsledky a upravit kalibraci podle důkazů. Co nebylo pozorováno (zejména dlouhodobý farmicí běh), uvést jako neověřené. Označit dokončené kroky v plánu.

## Primární technické reference

- SendInput a omezení oprávnění: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput
- HSV prahování OpenCV: https://docs.opencv.org/4.x/da/d97/tutorial_threshold_inRange.html

## Handoff

Implementace a lokální ověření: 54 testů, compileall a pip check prošly. Read-only kontrola odhalila elevated=True u klienta a elevated=False u bota. Start nově ověří oprávnění a aktivuje vybrané okno; pohyb používá jediný SendInput backend s čekáním na doručení. Živá aktivace, výběr a útok stále potřebují ověření. Zbývá ručně spustit launcher jako správce a potvrdit UAC, pak ověřit výběr, útok, doplňování, rotaci a F8. Podrobnosti jsou v docs/verification.md a progress.md.
