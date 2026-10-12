# Trasporto nativo e dispositivi

Oaclis gestisce rilevamento, regole, approvazioni e avvio delle operazioni. Non installa cron o timer systemd. Il servizio SSH esistente resta il trasporto: rsync su SSH esegue le copie. Nessun cloud o Git.

## Porta Oaclis

Scelta dall'utente: **23232/TCP**, predefinita e modificabile, uguale per master e client. Il futuro servizio esporrà UI, API di controllo, stato, controlli operativi e accettazione dell'abbinamento. Le comunicazioni fra dispositivi prevedono HTTPS e certificati verificati nell'abbinamento; il protocollo non è ancora implementato.

Il rilevamento mDNS usa il proprio protocollo e annuncia la porta Oaclis. SSH conserva la porta configurata per ciascun dispositivo: nelle due WSL di prova è esposto sulla LAN a 2222/TCP. rsync usa SSH. La porta 23232 non è stata aperta né esposta: quando sarà presente il servizio, il wizard dovrà gestire anche l'inoltro Windows/WSL e il firewall, come per SSH.

## API e comandi attuali

`devices` conserva ID, nome e alias SSH in un file JSON scritto atomicamente. Aggiungere un ID crea un dispositivo, riutilizzarlo ne modifica nome e collegamento mantenendo gli altri. IP, porta, utente e chiavi restano nella configurazione nativa SSH. Nessun dispositivo è fissato nel codice.

```sh
./target/debug/oaclis device-set /root/.config/oaclis/devices.json wsl-2 'Portatile WSL' oaclis-wsl-2
./target/debug/oaclis device-list /root/.config/oaclis/devices.json
./target/debug/oaclis send-file /root/.config/oaclis/devices.json wsl-2 /percorso/origine /percorso/remoto/file
```

`transfer::send_file` crea una copia privata locale mentre calcola SHA-256, controlla i metadati dell'origine, crea una cartella remota privata, invoca rsync e verifica dimensione e SHA-256 sul destinatario. Pubblica tramite hard link senza sovrascrivere una destinazione esistente, anche se compare dopo il controllo iniziale. Sincronizza il filesystem e ripulisce i temporanei. Conserva mtime e permessi ordinari, senza bit speciali. Non elimina l'origine.

Richiede Linux, SSH configurato e autenticato con chiave, rsync con `--secluded-args` e `--fsync`, e strumenti GNU standard sul destinatario. Non richiede un eseguibile Oaclis remoto per ricevere. Le directory di destinazione devono esistere e non attraversare link simbolici.

## Verifiche e limiti

20 test Rust superati; formattazione e clippy senza avvisi. `tools/test_rsync.py` ha verificato realmente entrambe le direzioni fra ROG e portatile: impronte, permessi, mtime, nomi con metacaratteri, file vuoti, rifiuto di destinazioni esistenti/link/cartelle mancanti e pulizia. Risultati in [rsync-test-results.json](rsync-test-results.json). Usati solo dati temporanei; archivi Codex preservati.

Questo passo implementa copie iniziali di singoli file. Aggiornamenti di singoli file/chat, checkpoint e conflitti sono ora collegati al trasporto: [SYNC.md](SYNC.md). Gli alberi di progetto restano da integrare. Due copie opposte non costituiscono un algoritmo di sincronizzazione bidirezionale.

La lettura dei metadati non blocca chi scrive sull'origine: trasferire file a riposo. Le directory di destinazione devono essere controllate dal dispositivo, senza cambiamenti concorrenti della struttura. Una disconnessione durante la pulizia può lasciare una cartella privata; durante la conferma può lasciare un file pubblicato con esito incerto. Controllare la destinazione prima di riprovare. L'editing concorrente della configurazione dispositivi non è coordinato. UI, servizio 23232 e automazione non sono ancora presenti.

Il trasporto espone anche `send_file_with_progress` con callback serializzabile; dettagli in [PROGRESS.md](PROGRESS.md). La copia reale di QR3D è documentata in [PROJECT-COPY.md](PROJECT-COPY.md).
