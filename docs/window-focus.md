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

Il toggle nel menu e previsto per il servizio dello sguardo, non per questo script finito. Il servizio webcam non e ancora implementato: nessun pulsante fittizio lo dichiara attivo. Lo script Win32 e specifico per Windows; il bordo della console funziona nei browser anche su macOS e Linux.
