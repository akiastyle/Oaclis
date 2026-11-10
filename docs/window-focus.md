# Prova del focus Windows

Prima verifica per il futuro controllo dello sguardo: nessuna webcam, OpenCV o stima dello sguardo. Il programma finale Rust non viene ancora installato. Questo script PowerShell usa direttamente le API Win32, senza cambiare impostazioni, registro o autorizzazioni.

Dalla cartella Oaclis, in PowerShell su Windows:

```powershell
.\scripts\focus-probe.ps1
```

Elenca solo finestre browser visibili, non minimizzate, con titolo Oaclis o `<chat> · Oaclis`. Gli altri programmi e le schede browser con un altro titolo non sono candidati. Restituisce handle, posizione, dimensioni e coordinate del desktop virtuale, comprese quelle negative.

Scegliere uno degli handle restituiti:

```powershell
.\scripts\focus-probe.ps1 -WindowHandle 0x1705F8
```

Dopo 500 ms lo script ricontrolla che la finestra sia ancora un candidato Oaclis e tenta `SetForegroundWindow`. Attende al massimo 700 ms per verificare `GetForegroundWindow` e il proprietario nativo del focus della tastiera tramite `GetGUIThreadInfo`. Non simula tasti, clic, Alt o cambio delle regole di Windows. `AlreadyForeground=false` distingue un vero cambio da una finestra che era gia attiva. Il focus nativo della tastiera non dimostra da solo che il cursore sia nel campo messaggio della pagina.

Per avere tempo di passare manualmente a un altro programma prima del tentativo:

```powershell
.\scripts\focus-probe.ps1 -WindowHandle 0x1705F8 -DelayMs 5000
```

`FOCUS_OK` indica primo piano e focus nativo confermati per quel tentativo. `FOCUS_NOT_CONFIRMED`, con uscita 2, indica che il controllo non li ha confermati. Un risultato positivo non garantisce tutti gli scenari: [Windows limita SetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow).

Nella UI, il bordo della singola console evidenzia il focus DOM effettivo dentro la pagina e si spegne quando la finestra perde il focus. La transizione dura 0,8 s e rispetta la preferenza di movimento ridotto. Funziona per pannelli interni e finestre separate; non aggiunge rilevamento dello sguardo o selezione automatica dei pannelli.

Nel menu Oaclis, l’icona occhio apre il pannello contestuale Controllo sguardo. La selezione della webcam viene ricordata nel browser; Avvia anteprima richiede il permesso della fotocamera e mostra solo il video locale, senza audio, registrazioni o invio di immagini. Ferma anteprima, la chiusura del pannello e l’uscita dalla pagina rilasciano la camera. La camera selezionata e quella del computer sul quale e aperto il browser, non quella di un peer selezionato nella mappa.

Calibra, Test e Focus automatico sono disabilitati e dichiarati non disponibili: il servizio di rilevamento dello sguardo non e ancora implementato. Lo script Win32 resta specifico per Windows; anteprima webcam e bordo della console usano API browser e funzionano anche su macOS e Linux compatibili.

Il pannello mostra anche una mappa dei monitor Windows del computer che esegue Oaclis: usa i rettangoli DPI-aware reali, non la disposizione del browser, e conserva coordinate negative e risoluzioni diverse. Il monitor principale e indicato nella mappa; cliccando sui monitor si salvano nel browser quelli da includere. Rileva monitor aggiorna la disposizione solo su richiesta, senza polling. Questa fase non raccoglie punti di calibrazione o misure degli occhi.
