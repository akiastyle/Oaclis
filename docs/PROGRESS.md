# Avanzamento per la UI

Il trasporto Rust espone `transfer::send_file_with_progress(device, source, destination, callback)`. `send_file` conserva la stessa API senza callback. Nessuna dipendenza aggiunta; gli eventi sono in `progress::Event` e sono serializzabili in JSON.

Fasi: `preparing`, `transferring`, `verifying`, `completed`. Un errore torna come `io::Error`: il chiamante lo mostra come stato fallito e non deve ricavare il successo dalla percentuale. `completed` arriva solo dopo la verifica remota SHA-256/dimensione, pubblicazione senza sovrascrittura e sincronizzazione del filesystem.

Durante rsync vengono letti subito stdout e separatori CR/LF con `--info=progress2,name0 --outbuf=N`, imponendo `LC_ALL=C`. La CLI emette gli eventi JSON su stderr; stdout resta la ricevuta finale. Su stderr possono comparire anche diagnostiche native SSH/rsync: la UI deve usare il callback, non interpretare quel canale come solo JSON.

```json
{"phase":"transferring","processed_bytes":1048576,"percent":25,"rate":"2.00MB/s","remaining_seconds":12,"total_known":true}
```

I byte sono dati logici elaborati da rsync, non il traffico reale SSH. La velocità resta nel formato nativo per evitare conversioni ambigue. La stima riguarda il trasferimento, non preparazione, verifica o durata totale. Le righe finali con `xfr#` riportano tempo trascorso: non viene presentato come ETA. Se `ir-chk` segnala un elenco ancora incompleto, `total_known=false` e la UI deve mostrare un avanzamento indeterminato. Se la stima non è disponibile, `remaining_seconds=null`.

La UI mostrerà una barra durante il trasferimento, una fase di verifica separata e successo solo alla conferma. Non inventare un conto alla rovescia per le fasi senza stima; l'ETA può oscillare con la rete. La scansione preliminare completa delle cartelle e un totale iniziale dei file permetteranno in seguito una percentuale più stabile.

Verificati parser, output non valido, tempo trascorso/ETA, totali incompleti ed eventi reali su trasferimenti SSH. Annullamento, pausa/ripresa, heartbeat durante hashing e avanzamento dettagliato della verifica sono ancora da implementare. Il primo modulo Rust gestisce singoli file; la copia QR3D resta una prova operativa di cartella con rsync nativo.

Fonte primaria: [manuale rsync, progress e buffering](https://download.samba.org/pub/rsync/rsync.1#opt--progress).
