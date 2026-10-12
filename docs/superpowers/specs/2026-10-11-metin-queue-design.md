> SUPERSEDED: historical queue-mode record. Active design: 2026-10-11-profile-target-design.md; runtime now uses individual clicks and target panels.

# Bot pro doplňování fronty metinů ve Verzum

## Cíl a ověřené vstupy

Lokální aplikace pro Windows doplňuje herní frontu metinů běžným ovládáním klienta `verzumclient_x64.exe`. Uživatel potvrdil kapacitu tří metinů, přidání levým kliknutím nebo Ctrl + tažením obdélníku a doplňování existující fronty opakovaným výběrem. Q/E otáčí kamerou, WASD pohybuje postavou.

Na dodaných obrázcích jsou metiny a žluté svislé paprsky označených cílů. Obrázek nedokazuje, že všechny cíle fronty musejí být viditelné nebo že právě napadený cíl má paprsek. Nepřítomnost paprsků proto nebude znamenat prázdnou frontu.

## První verze

- Výběr jednoznačného herního okna podle procesu; při více oknech vyžaduje výběr uživatele.
- Nastavitelný obdélník uvnitř herní plochy, mimo panely, minimapu a lištu schopností. Souřadnice se vztahují ke klientské ploše okna.
- Náhled snímku s oblastí výběru a detekovanými paprsky. Kontrola nejprve probíhá bez odesílání vstupů.
- Start, pauza a globální nouzové zastavení. Výchozí stav je zastaveno.
- Ctrl + levý drag doplní frontu. Četnost výběru má explicitní nastavitelnou mez; výběr se neopakuje v každém snímku.
- Po výběru se z dalších snímků vyhodnotí paprsky a změna scény. Výsledek může být potvrzené označení nebo nejistota; nebude se zobrazovat vymyšlený přesný počet členů fronty.
- Při dlouhodobě chybějícím potvrzení nebo postupu následuje krátké otočení kamery Q/E, ustálení obrazu a nový výběr. Po omezeném počtu neúspěšných pokusů se bot pozastaví a zobrazí důvod.
- Upřesnění z kontroly implementace: při neměnném označení po 120 s se bot pozastaví. Změna polohy/počtu stabilních paprsků je pouze heuristika postupu, nikoli potvrzené zničení. Interval doplnění platí pro všechny výběry včetně hledání po otočení.
- První verze neobsahuje samostatné hledání cesty přes WASD. Přiblížení k vybranému metinu zajišťuje hra.

## Jeden běh a jedna řídicí smyčka

Snímání okna -> vyhodnocení obrazu -> řídicí stav -> nejvýše jedna vstupní akce -> nový snímek. Řídicí stavy jsou zastaveno, sledování, doplnění, čekání po akci, otočení a chyba. Konfigurace má jediný zdroj a všechny akce používají stejnou implementaci ovládání.

Rozpoznávání paprsků využije jejich barvu, svislý tvar a stabilitu v několika snímcích. Samotné žluté číslo poškození se nesmí považovat za paprsek. Změna obrazu je podpůrná informace, nikoli důkaz zničení metinu: může ji způsobit animace, pohyb nebo jiný hráč. Časové limity představují pojistku proti nekonečnému opakování, nikoli měření obsazenosti fronty.

Konkrétní prahy a intervaly se nastaví podle zaznamenaných snímků a měření živého klienta. Budou uvedeny v konfiguraci a dostupné v náhledu. Bez takového měření nelze poctivě slíbit spolehlivost detekce.

## Ovládání a selhání

Vstupy se odesílají pouze do vybraného viditelného okna v popředí. Ztráta zaměření, minimalizace, změna velikosti okna nebo neplatný snímek pozastaví běh. Tlačítko Start na výslovný pokyn uživatele jednorázově aktivuje vybrané herní okno, bez kliknutí do světa. Během běhu bot zaměření nevynucuje. Před aktivací a vstupy kontroluje shodu zvýšených oprávnění; UAC potvrzuje uživatel ručně.

Každá akce uvolní Ctrl, tlačítko myši i klávesu otočení při dokončení, zastavení nebo chybě. Kontroluje platnost oblasti a souřadnic před vstupem. Snímky zůstávají lokálně; ukládání diagnostiky je explicitní volba. Nečte paměť hry ani neposílá vlastní herní pakety.

## Ověření před dokončením

1. Jednotkové kontroly řídicích přechodů, limitu akcí, zastavení a uvolnění vstupů při chybě.
2. Detekce na snímcích s paprsky, bez paprsků, s čísly poškození a s částečně viditelným cílem. Výstupem bude náhled umožňující posoudit chybné detekce.
3. Živá kontrola jednoho doplnění fronty během útoku: potvrdit, že hra útok nepřeruší a duplicitní výběr nemění již zařazené cíle.
4. Živá kontrola doplňování, otočení při neúspěchu a pauzy při ztrátě zaměření. Start delšího běhu až po těchto kontrolách.
5. Spustit relevantní testy a kontrolu syntaxe podle zvoleného technologického řešení. Odděleně uvést automaticky ověřené části a chování ověřené v klientovi.

## Navazující implementační plán

Po odsouhlasení návrhu vybrat jednoduchý lokální technologický základ a sepsat implementační kroky pro snímání, ovládání, řídicí smyčku, konfiguraci, náhled a ověření. Další funkce, například pohyb po mapě nebo jiné druhy metinů, přidávat pouze podle skutečných výsledků první verze.
