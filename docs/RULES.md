# Cartelle e regole salvate

Prima si indicava ogni file da aggiornare. Ora si salva una scelta: dispositivo, due cartelle, copia oppure sincronizzazione, direzione e file inclusi/esclusi. Una sola operazione applica la scelta a tutti i file del progetto. Salvare o modificare una regola **non avvia operazioni automatiche**.

`rules` conserva e valida queste scelte, enumera i file, applica `ProjectPaths` e chiama il motore `sync` esistente. Per le copie usa lo stesso trasporto verificato senza sovrascritture. Nessuna dipendenza aggiunta, nessun Git come trasporto, cron o timer.

## Definizione

```json
{
  "id": "qr3d",
  "name": "QR3D",
  "device_id": "wsl-2",
  "local_root": "/var/www/html/qr3d",
  "remote_root": "/var/www/html/qr3d",
  "operation": "sync",
  "direction": "both",
  "include": [],
  "exclude": ["_old"]
}
```

Questa regola è stata salvata sul master in `/root/.config/oaclis/rules.json`, conservando eventuali altre regole. Non è stata eseguita su QR3D: le verifiche hanno usato soltanto progetti temporanei. `_old` è esclusa dagli aggiornamenti perché contiene riferimenti protetti dalle istruzioni del progetto, già inclusi nella prima copia.

`operation` può essere `copy` o `sync`. `direction` può essere `send` o `both`; la copia ammette solo `send`. La copia trasferisce i file assenti, salta quelli identici e segnala conflitti per destinazioni differenti. La sincronizzazione usa le basi comuni e i backup già implementati. In modalità `send`, modifiche remote incompatibili diventano conflitti: non vengono ricevute o scartate.

Le inclusioni/esclusioni sono percorsi relativi di file o cartelle, confrontati per componenti: `src` comprende i suoi discendenti e non `src2`. Nessun glob; le selezioni vuote in `include` significano tutto il progetto. Questo modello potrà essere alimentato da checkbox e drag&drop senza digitare filtri.

Sono sempre esclusi `.git`, `.ssh`, `.codex`, `node_modules`, `target`, `__pycache__`, nomi `.oaclis-*`, `gpg*.txt`, `.env`/`.env.*`, `.htaccess`, `.pem`, `.key` e `.pyc`. Questi filtri proteggono i nomi previsti, non riconoscono automaticamente ogni possibile file contenente segreti. Gli archivi SSH/Codex non possono essere radici di progetto; le chat mantengono il proprio flusso separato. Radici del filesystem, percorsi esterni, ID duplicati, campi sconosciuti e selezioni con `..` sono rifiutati.

## CLI

```sh
./target/debug/oaclis rule-set REGOLE DEFINIZIONE_JSON
./target/debug/oaclis rule-list REGOLE
./target/debug/oaclis rule-run DISPOSITIVI REGOLE ID STATO
```

`rule-set` aggiunge o modifica lo stesso ID, conservando le altre regole; il salvataggio JSON è atomico. `rule-run` legge il dispositivo dal registro: indirizzi e chiavi restano nella configurazione SSH. Il cambio dell'alias/percorso produce checkpoint distinti, evitando di riutilizzare una base di un'altra coppia.

Lo stato deve stare fuori dal progetto. Viene creato se necessario e contiene un checkpoint per coppia di file, con nomi SHA-256. In modalità `both` l'elenco comprende anche i file nuovi sul remoto: i file mai sincronizzati possono essere ricevuti quando manca la copia locale. Un file precedentemente sincronizzato e poi eliminato produce un conflitto, senza cancellazioni propagate o ricreazione automatica.

Le directory necessarie vengono create. I percorsi attraversati non possono essere link; link e file speciali selezionati bloccano la scansione. Non vengono sincronizzate le directory vuote, i proprietari, ACL o attributi estesi. I percorsi remoti devono essere UTF-8. L'enumerazione remota usa `find` GNU con separatori NUL, quindi preserva spazi, apostrofi e newline.

## Avanzamento ed esiti

Il callback espone `ItemProgress` con percorso relativo, indice del file, totale dei file e l'evento del trasporto. La UI potrà indicare “file 3 di 12” e mostrare il progresso rsync del file corrente. `execute` restituisce l'esito di ogni file: inviato, ricevuto, identico o conflitto. Il totale dei file non è una percentuale pesata per byte; una stima complessiva del progetto non è ancora calcolata.

I conflitti non impediscono l'aggiornamento degli altri file indipendenti, ma impediscono di considerare tutta la cartella allineata. Un errore operativo ferma l'esecuzione: gli aggiornamenti già confermati restano applicati e i relativi checkpoint sono salvati. Una nuova esecuzione riparte dai contenuti effettivi. La cartella intera non è una transazione atomica.

## Verifiche e limiti

23 test Rust superati, formattazione e clippy senza avvisi. La diagnostica `tools/test_rules.py` (strumento solo locale) ha verificato sulle due WSL: regole salvate senza esecuzione, cartelle annidate, nomi con metacaratteri/newline, inclusioni/esclusioni, nuovi file nelle due direzioni, conflitti preservati, modalità monodirezionale e copia, modifica delle regole mantenendo le altre e cancellazioni non propagate. Risultati in [rules-test-results.json](rules-test-results.json).

Usare progetti a riposo e una sola operazione per coppia. Le copie sono sequenziali e possono richiedere tempo su molti file; non c'è ancora batching o parallelismo. L'enumerazione remota legge i nomi anche nelle sottocartelle escluse, senza trasferirne il contenuto. Permangono i limiti contro scrittori esterni e disconnessioni descritti in [SYNC.md](SYNC.md).

Restano da collegare chat e progetto in una singola regola, distribuzione delle regole ai peer, approvazioni/automazione, servizio 23232 e UI. L'inferenza automatica dei percorsi dalla conversazione non è ancora implementata.
