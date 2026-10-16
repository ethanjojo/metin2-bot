> Historical verification report. Use the repository README and docs/release-readiness.md for current installation/release status. Earlier commands, counts and package paths below describe their original snapshots.

# Interaktivní editor

Uživatelský požadavek: jasně vidět aktuální výběr, posouvat a měnit oblasti tažením, jednodušší kalibrace. Rozšíření existujícího nativního toku, bez změny pravidel automatického útoku.

Jedna kalibrace vlastní immutable Profile a zmrazený frame. Gesture drží pouze originální region a klientské souřadnice; náhled tažení nemění profil. Commit při mouse-up prochází stejným Calibration.set_region jako vytvoření oblasti. Escape zahodí náhled. Masky mají index; edit nahradí jeden region, delete odstraní jeden. Template region se ukládá v JSON, PNG zůstává skutečná šablona používaná runtime.

V hlavním zobrazení jsou kalibrační kroky se stavem, aktivní titul/nápověda, označené oblasti s úchyty a neuložené změny. Pokročilé parametry v samostatné záložce. Sidebar lze posouvat kolečkem při menším okně. Při tažení se překreslují jen vektory na Canvas, nikoli barevná detekce celého snímku.

Ověření: tests/test_editing.py, tests/test_app.py a celé unittest; vlastní UI fixture z dodaných snímků, bez herních vstupů. Finální review a nálezy viz profile-progress.md a verification.md.
