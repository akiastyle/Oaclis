# Prove fra due WSL nella LAN

## Collegamento verificato

Origine: WSL del computer ROG. Destinazione: WSL `DESKTOP-U8M8SOU`, raggiunta tramite `root@192.168.1.167:2222`.

La chiave host è stata verificata confrontando l'impronta rilevata con quella mostrata dall'utente sul dispositivo. L'utente ha autorizzato la chiave pubblica dedicata di Oaclis per root. Il login BatchMode con chiave è riuscito.

Il collegamento locale è salvato nell'alias SSH `oaclis-wsl-2`; usa `/root/.ssh/id_ed25519_oaclis` e il file host verificati `/root/.config/oaclis/known_hosts`. Gli accessi preesistenti sono stati conservati. L'utente ha autorizzato root per installazione e configurazione; appartenenza al gruppo root non è equivalente a privilegi root.

L'utente considera l'archivio del portatile una destinazione di prova sostanzialmente vuota. Le prove effettuate hanno comunque usato directory temporanee private, senza cancellare o importare dati nei suoi archivi Codex abituali di root o kim.

## Prova riproducibile

```bash
cargo build --locked
python3 tools/test_ssh.py oaclis-wsl-2
```

[test_ssh.py](../tools/test_ssh.py) invia la CLI Rust compilata e dati dimostrativi in una directory remota creata con mktemp. Verifica:

- Copie chat inizialmente identiche.
- Rilevamento di nuovi record come SourceAhead.
- Aggiornamento della copia dimostrativa e recupero con contenuto identico.
- Invio e recupero di un file di progetto dimostrativo.
- Divergenza come Conflict, destinazione assente e rifiuto di una chat troncata.
- Corrispondenza fra percorsi locali e remoti.

Invia inoltre copie stabili di due chat principali reali, legacy e paginated, e la diagnostica probe_codex.py. Sulla macchina remota la diagnostica crea un CODEX_HOME temporaneo, blocca la rete del processo Codex e verifica elenco, lettura, ripresa e directory rimappata. Non avvia turni del modello.

Esiti in [ssh-test-results.json](ssh-test-results.json): tutti i controlli riusciti. Il campione legacy è stato ripreso con 26 turni e 75 elementi; quello paginato con 3 turni e 46 elementi. I file sorgente sono rimasti invariati. Le directory di prova vengono rimosse al termine.

Lo script è diagnostico e usa il binario locale x86_64 Linux, verificato su questa seconda WSL. Non è un trasporto di produzione: non effettua sincronizzazione automatica, ripresa dopo disconnessione o gestione delle copie già aperte da Codex.

## Accesso inverso verificato

È stata generata una chiave Oaclis sul portatile in `/root/.ssh/id_ed25519_oaclis`. Solo la pubblica è stata recuperata e aggiunta agli authorized_keys della WSL locale, conservando gli accessi preesistenti.

Nella WSL locale è stato installato openssh-server e avviato SSH: listener su IPv4/IPv6 porta 22, autenticazione pubblica abilitata e login root senza password. La chiave host locale è stata fornita al portatile attraverso il collegamento già verificato, nel file `/root/.ssh/known_hosts_oaclis_rog`.

Indirizzi locali osservati: Windows Ethernet 2 `192.168.1.138`, WSL `172.20.94.232`. Windows ha già un inoltro HTTPS `192.168.1.138:443` → `172.20.94.232:9443`, da conservare. La porta 2222 non risultava in ascolto. La sessione PowerShell disponibile da qui ha restituito `Administrator=False`: l'inoltro inverso richiede un comando dell'utente in PowerShell amministratore sul computer ROG.

Comandi forniti per predisporre l'accesso inverso:

```powershell
netsh interface portproxy add v4tov4 listenaddress=192.168.1.138 listenport=2222 connectaddress=172.20.94.232 connectport=22
New-NetFirewallRule -Name "Oaclis-WSL-SSH" -DisplayName "Oaclis WSL SSH" -Direction Inbound -Action Allow -Protocol TCP -LocalAddress 192.168.1.138 -LocalPort 2222 -RemoteAddress LocalSubnet -Profile Any
netsh interface portproxy show all
```

L'utente ha mostrato entrambi gli inoltri Windows attivi: il preesistente HTTPS sulla 443 e il nuovo SSH sulla 2222. Il login iniziato dalla seconda WSL verso `root@192.168.1.138:2222` è riuscito e ha restituito `root`, `ROG`, `x86_64`, con controllo rigoroso della chiave host e autenticazione tramite chiave Oaclis remota.

Sono riusciti anche un invio e un recupero SCP iniziati dalla seconda WSL, su un file dimostrativo in directory temporanee private. Il contenuto ricevuto e l'impronta SHA-256 della copia recuperata coincidono con l'originale. Le directory di prova sono state rimosse. Esiti in [ssh-reverse-results.json](ssh-reverse-results.json).

SSH e trasferimenti sono quindi verificati nelle due direzioni. Restano da implementare le regole Oaclis, il trasporto di produzione, ripresa dopo disconnessione, gestione degli archivi Codex abituali e wizard; queste prove non equivalgono ancora a una sincronizzazione automatica.

## Prima copia richiesta nell'archivio reale

Copiata tramite il trasporto Rust/rsync la chat `01a0568c-cf6c-7d62-bb6d-12c20b4d9b16` dal master ROG alla WSL del portatile, nell'archivio di `root`. Nessuna cancellazione della sorgente e nessun altro archivio sostituito. SHA-256 coincidente al trasferimento; sorgente ricontrollata e invariata.

La ripresa nativa con app-server sul portatile, senza rete e senza avviare turni del modello, ha ricostruito 1006 turni e 11024 elementi, coincidenti con il database sorgente. Codex ha aggiunto alla copia un solo evento `thread_settings_applied` di 2017 byte; il prefisso originale è integralmente preservato. Non sono stati copiati file di progetto, credenziali o configurazioni del master. Dettagli in [chat-copy-01a0568c.json](chat-copy-01a0568c.json).
