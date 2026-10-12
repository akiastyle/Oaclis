# Nucleo Rust

Il package Cargo contiene una libreria e una piccola CLI diagnostica. Tutte le operazioni attuali leggono e confrontano: non importano chat, non scrivono progetti e non aprono connessioni di rete.

## Moduli

| Modulo | Responsabilità | API |
|---|---|---|
| `chat` | Validazione JSONL, identità, formato, dipendenza dal padre e directory di lavoro proposte | `Chat::read`, `Chat::record_count` |
| `paths` | Corrispondenza esplicita fra una cartella origine e una destinazione | `ProjectPaths::new`, `ProjectPaths::map` |
| `compare` | Confronto conservativo di chat e contenuto dei file | `chats`, `files` |

`lib.rs` esporta i moduli del nucleo e contiene solo i piccoli controlli condivisi di lettura dei file. `main.rs` passa gli argomenti alle API. Interfaccia e trasporto possono usare la libreria senza dipendere dalla CLI. Non sono presenti framework GUI, runtime asincroni, database aggiuntivi o servizi.

Dipendenze dirette: `serde_json` per validare JSON e `sha2` per impronte SHA-256. Le versioni risolte sono conservate in `Cargo.lock`. Il download degli strumenti e delle dipendenze riguarda la compilazione; il nucleo eseguibile funziona offline.

## Chat e percorsi

Ogni record deve essere JSON valido, avere un tipo e un payload oggetto. Il primo record deve essere `session_meta`, con ID e directory assoluta. I formati supportati sono `legacy` e `paginated`; in assenza del campo si usa `legacy`, come nei campioni precedenti. Un record finale senza newline viene rifiutato conservativamente, anche se contiene JSON valido: potrebbe provenire da una scrittura in corso. Non correggere o tagliare silenziosamente una coda danneggiata.

Le directory proposte provengono da `session_meta.cwd` e `turn_context.cwd`. I percorsi citati liberamente nei messaggi o nei comandi non sono ancora estratti. Le proposte non autorizzano la copia delle cartelle: la selezione resta esplicita.

`ProjectPaths` associa due radici assolute. Rifiuta percorsi relativi, componenti `..`, byte NUL, radice del filesystem come progetto e file fuori dalla radice scelta. Il controllo è per componenti: `/work/app2` non appartiene a `/work/app`. È una corrispondenza lessicale; non segue o risolve link simbolici e non riscrive i messaggi storici.

## Confronto delle chat

Ogni record viene analizzato e ne viene calcolata un'impronta SHA-256 sul JSON riserializzato, conservando soltanto le impronte in memoria. Ordine dei campi e spaziatura non generano differenze. I metadati e tutti i record restano parte del confronto.

| Risultato | Significato |
|---|---|
| `DestinationMissing` | Non è stata fornita una copia di destinazione |
| `Identical` | Stesso ID e stessa sequenza di record |
| `SourceAhead` | Tutti i record della destinazione sono un prefisso di quelli dell'origine |
| `DestinationAhead` | Tutti i record dell'origine sono un prefisso di quelli della destinazione |
| `Conflict` | Stesso ID ma record divergenti |
| `DifferentChat` | ID diversi |

Nessun risultato avvia una copia. Non vengono usati orologio, timestamp o numero di messaggi per decidere cosa sovrascrivere. Cambiamenti ai metadati, inclusa una directory riscritta nell'intestazione, producono conservativamente un conflitto: non è ancora definita una normalizzazione fra dispositivi.

## Confronto dei file

Legge i file a blocchi e confronta SHA-256. Restituisce `Identical`, `Different` o `DestinationMissing`. Un'origine mancante, un file non leggibile, una directory o un link simbolico finale produce un errore. Una destinazione differente non viene considerata più vecchia o sovrascrivibile: per stabilire la direzione servirà una base condivisa di sincronizzazione.

## Esecuzione

Rust è installato in questa macchina tramite rustup, senza modificare automaticamente i file della shell. Se Cargo non è nel PATH:

```bash
source /root/.cargo/env
```

Dalla cartella `/root/Oaclis`:

```bash
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo run --locked -- inspect /percorso/chat.jsonl
cargo run --locked -- compare-chat /origine/chat.jsonl /destinazione/chat.jsonl
cargo run --locked -- compare-file /origine/file /destinazione/file
cargo run --locked -- map-path /origine/progetto /destinazione/progetto /origine/progetto/src/main.rs
```

Un confronto valido termina con codice 0 anche quando segnala un conflitto; errori di lettura, validazione o argomenti terminano con codice 1. `inspect` stampa soltanto metadati e conteggi, non i messaggi della chat.

## Verifiche e limiti

I test coprono tutti gli esiti del confronto chat, la spaziatura JSON, cronologie troncate, metadati duplicati, file differenti della stessa dimensione, file mancanti, link simbolici, percorsi esterni e nomi con spazi. La CLI è stata verificata in sola lettura anche sui due campioni reali legacy e paginated.

La verifica dei metadati del file prima/dopo la lettura intercetta cambiamenti ordinari ma non costituisce uno snapshot o un lock. Prima di copiare chat attive occorre coordinarne la consistenza. La mappatura dei percorsi non è una protezione contro cambiamenti concorrenti o link simbolici nelle directory antenate: tali controlli devono accompagnare il futuro trasporto.

Non sono ancora implementati catalogo completo, estrazione dei percorsi dal testo, scelta file/esclusioni, importazione in Codex o GUI. I moduli aggiunti `devices` e `transfer` gestiscono dispositivi e copie iniziali tramite rsync su SSH; `ssh` e `staging` sono helper interni. Comandi e limiti in [RSYNC-TRANSPORT.md](RSYNC-TRANSPORT.md).

`progress` espone eventi serializzabili delle fasi di copia; `transfer::send_file_with_progress` li notifica senza dipendere dalla UI o da un runtime asincrono. API e limiti in [PROGRESS.md](PROGRESS.md).

Il modulo `sync` collega confronto, checkpoint e trasporto per gli aggiornamenti di singoli file/chat. `staging::save_json` è condiviso dal registro dispositivi e dai checkpoint; API, criteri e limiti in [SYNC.md](SYNC.md).

`rules` conserva le selezioni del progetto e applica il motore a una cartella. `ItemProgress` aggiunge percorso e conteggio dei file agli eventi del trasporto; dettagli in [RULES.md](RULES.md).
