# Prova desktop remoto (separata da Oaclis)

Stato: preparazione del client, non integrazione nella mappa o nelle autorizzazioni Oaclis.
Nessun avvio normale di Oaclis installa o apre RustDesk.

Dal repository Git sul computer interessato:

- Windows / WSL: `powershell -ExecutionPolicy Bypass -File .\run.ps1 -DesktopTest`, oppure in WSL `bash run.sh --desktop-test`.
- Linux con desktop / Mac: `bash run.sh --desktop-test`.
- Solo verifica senza installare: `bash scripts/desktop-test.sh --check`, oppure `powershell -ExecutionPolicy Bypass -File .\scripts\desktop-test.ps1 -Action Check`.

Windows usa il pacchetto RustDesk.RustDesk della sorgente winget, macOS il cask Homebrew.
Linux Debian/Ubuntu usa la release ufficiale 1.5.0 x86_64 o ARM64 con SHA256 fissato.
Linux senza sessione X11/Wayland non installa niente; su WSL si prepara sempre Windows.
Il passaggio WSL usa ExecutionPolicy Bypass solo nel processo PowerShell figlio, senza cambiare la policy salvata.
Le autorizzazioni del sistema operativo restano necessarie: UAC su Windows,
Registrazione schermo e Accessibilita su Mac, eventuale consenso alla cattura su Wayland.
Non vengono modificati compositor, firewall, inoltri, password, autenticazione o stato Oaclis.
Il pacchetto nativo puo predisporre il proprio servizio RustDesk: resta separato da Oaclis.

## Test ROG -> KDell

1. Preparare i due host Windows con lo script.
2. Aprire RustDesk dal menu Start sui due computer.
3. Dal master ROG inserire l'ID mostrato su KDell, poi approvare su KDell la richiesta di ROG.
4. Controllare che sia lo stesso desktop del monitor KDell; provare mouse, tastiera e cambio monitor.
5. Chiudere la sessione dal master; non impostare accesso permanente durante il test.

Il prototipo usa il servizio pubblico RustDesk per individuazione/relay; non la relay mobile Oaclis.
Non attiva l'accesso diretto per IP o password permanenti e non apre porte nel router.
Le impostazioni RustDesk gia presenti vengono conservate: controllare quelle preesistenti prima del test.
L'approvazione di questa prova e' manuale sul device. Il vincolo tecnico "solo master Oaclis"
non e' ancora applicato al client esterno: deve essere verificato e implementato prima dell'integrazione.
Nessuna azione desktop viene esposta al telefono o alle API Oaclis in questa fase.

## Esito da verificare insieme

Desktop reale, input funzionante, monitor disponibili, latenza e comportamento alla chiusura.
La sola installazione non dimostra che il collegamento video funzioni sul device.

Fonti: https://rustdesk.com/docs/en/client/ e https://github.com/rustdesk/rustdesk/releases/tag/1.5.0
