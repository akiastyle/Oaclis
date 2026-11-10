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

Nel menu Oaclis, l’icona occhio apre il pannello contestuale Controllo sguardo. La selezione della webcam viene ricordata nel browser; L’apertura del pannello avvia l’anteprima, richiede il permesso della fotocamera e mostra solo il video locale, senza audio, registrazioni o invio di immagini. La X del pannello e l’uscita dalla pagina rilasciano la camera; non esiste un pulsante separato per avviare o fermare l’anteprima. La camera selezionata e quella del computer sul quale e aperto il browser, non quella di un peer selezionato nella mappa.

Calibra si abilita con anteprima attiva e almeno un monitor selezionato. Test e Focus automatico restano disabilitati: non viene ancora applicato alcun cambio di finestra tramite lo sguardo. Lo script Win32 resta specifico per Windows; anteprima webcam e bordo della console usano API browser e funzionano anche su macOS e Linux compatibili.

Il pannello mostra anche una mappa dei monitor Windows del computer che esegue Oaclis: usa i rettangoli DPI-aware reali, non la disposizione del browser, e conserva coordinate negative e risoluzioni diverse. Il monitor principale e indicato nella mappa; cliccando sui monitor si salvano nel browser quelli da includere. Rileva monitor aggiorna la disposizione solo su richiesta, senza polling. La selezione definisce i monitor inclusi nella calibrazione.


## Calibrazione manuale degli occhi

Aprendo il pannello la webcam si accende automaticamente. Calibra apre una finestra separata. Trascinare due rettangoli distinti, uno intorno a ciascun occhio, includendo pupilla e parte bianca ma evitando sopracciglia. L’immagine di selezione non e specchiata. La testa deve restare nella stessa posizione durante tutta la prova.

Continua richiede a Chrome il permesso di gestione dei monitor. Per ciascun monitor Windows selezionato si conferma lo schermo del browser corrispondente prima di Avvia i 9 punti. L’abbinamento iniziale usa geometria e DPI quando coincide; il selettore consente la conferma manuale, necessaria quando le coordinate browser e Windows differiscono. I bersagli sono presentati a schermo intero tramite [Window Management API](https://developer.chrome.com/docs/capabilities/web-apis/window-management), uno schermo alla volta. Il controllo resta separato dal focus automatico delle console.

Guardare il punto e premere Spazio (oppure cliccare il punto). Dopo 300 ms di assestamento si acquisiscono 12 misure distanziate di almeno 100 ms; servono almeno 8 misure valide e coerenti. Un errore ripropone lo stesso punto, senza aggiungerlo alla calibrazione. Esc o la X annullano; un cambiamento della disposizione dei monitor interrompe la prova.

La prima implementazione usa un rilevatore geometrico scritto direttamente in Rust, senza dipendenze OpenCV: filtraggio 3×3, soglia di intensita e componenti connesse compatte nelle due regioni selezionate. Non usa AI, classificatori addestrati o servizi esterni. Il browser invia esclusivamente due ritagli in scala di grigi 80×40 al servizio Rust locale, limitatamente ai punti acquisiti. I ritagli vengono elaborati e scartati, senza log o salvataggio.

Le coordinate medie delle pupille alimentano una regressione quadratica a sei termini verso il desktop virtuale Windows. La verifica leave-one-out scarta misure degeneri o errori eccessivi; l’errore mostrato e una stima sui punti raccolti, non una garanzia di precisione reale. I dati salvati nel browser contengono solo dispositivo video, rettangoli occhi, geometria monitor, coppie numeriche e coefficienti. Una calibrazione fallita o annullata non sovrascrive quella precedente. Gli spostamenti della testa e il contrasto insufficiente della webcam possono richiedere una nuova selezione o calibrazione. La verifica reale con la webcam dell’utente resta necessaria prima di attivare il focus automatico.

Controllo sguardo resta aperto quando si clicca fuori o si preme Esc; si chiude con la X. Il pulsante occhio riporta il focus al pannello gia aperto. La finestra di calibrazione e la webcam non vengono quindi interrotte da un clic esterno. Il pannello Collega dispositivo si apre solo su richiesta, senza apertura automatica alla scoperta di un peer.
