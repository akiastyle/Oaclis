# Aggiornamenti sicuri di singoli file e chat

`sync` decide la direzione; `transfer` usa rsync/SSH e pubblica le copie; `staging` conserva snapshot privati e scrive configurazioni/checkpoint atomicamente. Nessuna nuova dipendenza, nessun cron o timer. Dispositivi, UI e trasporto restano separati.

## Comandi

```sh
./target/debug/oaclis sync-file CONFIG ID LOCALE REMOTO CHECKPOINT
./target/debug/oaclis sync-chat CONFIG ID LOCALE REMOTO CHECKPOINT
```

CONFIG è il registro dispositivi esistente; LOCALE, REMOTO e CHECKPOINT sono percorsi assoluti. La directory del checkpoint deve esistere. Ogni coppia di file ha un checkpoint separato; non riutilizzarlo per altri percorsi, tipi o dispositivi. La CLI emette `"sent"`, `"received"`, `"identical"` oppure `"conflict"`. Un conflitto è un risultato da presentare all'utente, non un'autorizzazione a sovrascrivere. Errori operativi producono exit code 1.

Il checkpoint registra dispositivo, alias SSH, percorsi, tipo e impronta/dimensione della base comune. Viene aggiornato soltanto dopo il successo o il riconoscimento di copie identiche. Resta locale al dispositivo che avvia l'operazione: distribuzione delle regole e coordinamento fra peer non sono ancora implementati.

## Decisione

Per i file:

- Copie identiche: registra la base comune.
- Destinazione assente alla prima copia: invia.
- File diversi senza base comune: conflitto.
- Solo il locale differisce dalla base: invia.
- Solo il remoto differisce dalla base: riceve.
- Entrambi differiscono e sono diversi: conflitto.
- Una copia precedentemente sincronizzata scompare: nessuna cancellazione propagata o ricreazione automatica.

Per le chat: valida i JSONL e confronta ID e prefissi della cronologia normalizzata. Trasferisce la copia più avanti solo quando l'altra è un suo prefisso. ID diversi, JSON incompleto o rami divergenti bloccano l'aggiornamento. Non fonde messaggi né ignora eventi di configurazione aggiunti da Codex: tali eventi possono rendere divergenti due copie anche senza nuove risposte.

## Trasporto e recupero

Prima di inviare viene ricontrollata l'impronta dell'origine rispetto allo snapshot usato per decidere. Prima della pubblicazione viene verificato che la destinazione abbia ancora il contenuto atteso. Gli aggiornamenti riutilizzano una copia privata del file remoto come base delta di rsync, con `--checksum` per riconoscere modifiche anche a parità di dimensione e mtime. Per confrontare file ordinari basta l'impronta remota; la copia completa viene scaricata solo quando serve ricevere. Il confronto chat scarica invece un JSONL di prova.

SHA-256 e dimensione vengono verificati prima della pubblicazione. Le prime copie restano senza sovrascrittura; gli aggiornamenti sostituiscono il file tramite rename sullo stesso filesystem. Un lock nativo `flock` serializza la pubblicazione degli aggiornamenti Oaclis sullo stesso target.

La versione precedente viene copiata indipendentemente e conservata, con nome derivato dal percorso e dall'impronta, nella cartella privata:

```text
CARTELLA_DEL_FILE/.oaclis-backups/HASH_DEL_PERCORSO/SHA256_PRECEDENTE
```

Non vengono cancellati automaticamente backup o originali. Il callback della sincronizzazione emette `completed` dopo il salvataggio del checkpoint; su errore o conflitto il chiamante deve mostrare il relativo risultato. Il progresso rsync durante il confronto di una chat riguarda anche il download dello snapshot, non soltanto l'aggiornamento finale.

## Verifiche e limiti

23 test Rust superati, formattazione e clippy senza avvisi. Prove LAN su dati temporanei nelle due WSL: invio, ritorno, contenuto cambiato con dimensione/mtime identici, backup indipendenti, conflitti senza perdita, checkpoint vincolato ai percorsi, chat aggiornate nelle due direzioni, cronologie divergenti e troncate. Verificato un fallimento rsync simulato, seguito da un nuovo tentativo riuscito. Esiti in [sync-test-results.json](sync-test-results.json), diagnostica `tools/test_sync.py` (strumento solo locale). Nessuna modifica alla chat reale o a QR3D durante queste prove.

Usare file a riposo e directory controllate dai dispositivi. Il lock coordina Oaclis, ma Codex/editor esterni non lo rispettano: non aggiornare chat aperte. Il controllo hash non costituisce un compare-and-swap del filesystem contro scrittori esterni. Gli aggiornamenti di chat paginate riguardano qui il rollout; invalidazione dei database/cache Codex e ripresa dopo un aggiornamento reale sono ancora da verificare.

Una perdita di connessione dopo la pubblicazione può lasciare l'esito incerto: ricontrollare le copie prima di riprovare. Il confronto identico permette di ristabilire la base dopo un esito non confermato. Una disconnessione LAN reale non è stata provocata. Snapshot/backup richiedono spazio aggiuntivo; i backup non hanno ancora una politica di pulizia. Il checkpoint non ha un coordinamento distribuito: una sola operazione per coppia alla volta.

Il modulo `rules` ora applica questo motore alle cartelle e gestisce nuovi file remoti: [RULES.md](RULES.md). Non sono ancora presenti propagazione delle cancellazioni, risoluzione interattiva dei conflitti, servizio 23232, discovery o automazione.

`synchronize_with_direction` consente di limitare il flusso a `send`; `synchronize` mantiene il comportamento bidirezionale. Una copia locale assente senza checkpoint può ricevere il file remoto, senza sovrascritture; con un checkpoint precedente resta un conflitto.
