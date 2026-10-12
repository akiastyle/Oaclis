# Prima verifica di compatibilità Codex

Data: 10 ottobre 2026. Ambiente di riferimento: installazione locale Codex 0.162.0 dichiarata dall'utente.

## Metodo

Lo script diagnostico [probe_codex.py](../tools/probe_codex.py) usa soltanto la libreria standard Python. È uno strumento di verifica preliminare, non il motore di Oaclis, che sarà Rust.

Seleziona la più piccola chat principale non archiviata per ciascuno dei due formati presenti nell'installazione, legge il database in modalità read-only e copia il solo file JSONL in una directory temporanea privata. Non copia credenziali, configurazione personale, database o chiavi SSH.

Avvia un app-server Codex separato con HOME e CODEX_HOME temporanei. `unshare -n` disabilita la rete del processo. Esegue initialize, thread/list, thread/read, thread/resume con una directory di progetto temporanea e thread/read dopo la ripresa. Non esegue turn/start, non genera risposte, non avvia nuove attività o sottoagenti. Arresta il processo e rimuove le copie al termine.

Controlla l'identità della chat, la presenza nell'elenco, la rimappatura della directory di lavoro, la presenza della cronologia e l'immutabilità del file originale. Per il formato paginato confronta anche il numero di turni e di elementi con il database della sorgente. Il risultato salvato contiene soltanto esiti e conteggi, non il contenuto delle conversazioni.

Comando dalla cartella Oaclis:

```bash
python3 tools/probe_codex.py
```

La prova richiede Linux, Codex nel PATH, permesso di creare un namespace di rete e i database locali nel formato osservato. Usare chat a riposo; la diagnostica rileva cambiamenti del file durante la prova e non tenta di bloccare Codex.

## Risultato osservato

| Controllo | Legacy | Paginated |
|---|---|---|
| Copia del solo JSONL rilevata nell'elenco | Sì | Sì |
| Cronologia prima della ripresa | 26 turni, 75 elementi | 0 turni, 0 elementi |
| Ripresa tramite API locale | Sì | Sì |
| Directory di lavoro rimappata | Sì | Sì |
| Cronologia dopo la ripresa | 26 turni, 75 elementi | 3 turni, 46 elementi |
| Confronto con database sorgente paginato | Non applicabile | 3 turni, 46 elementi: coincidenti |
| File originale invariato | Sì | Sì |
| Rete disponibile al processo di prova | No | No |

Esiti riproducibili in [codex-probe-results.json](codex-probe-results.json).

La copia di un JSONL è quindi una base promettente per importare una chat principale. Nel formato paginato, la sola lettura prima della ripresa non dimostra che la cronologia sia assente: nella prova, la ripresa ha ricostruito i turni e gli elementi.

Una prova preliminare su una chat di sottoagente ha restituito un errore che richiede la ripresa del padre. Il trasferimento di queste chat va gestito come dipendenza della chat principale, non come chat indipendente.

## Conseguenze per Oaclis

- Integrare l'app-server locale di Codex come primo candidato per elenco, lettura e ripresa; non partire dalla scrittura diretta delle tabelle interne.
- Trasferire una conversazione selezionata senza sovrascrivere l'intero stato Codex della destinazione.
- Prevedere una fase di verifica/ripresa dopo l'importazione e prima di dichiarare il trasferimento completato.
- Mantenere l'ID della chat e registrare il percorso di progetto sulla destinazione. La directory passata alla ripresa non riscrive automaticamente tutti i percorsi nei vecchi messaggi o comandi.
- Separare metadati aggiuntivi, allegati, relazioni padre/figlio e file di progetto dalla prova minima JSONL.
- Una copia già esistente sulla destinazione richiede confronto e gestione dei conflitti: questa prova usa una destinazione vuota.

## Limiti della verifica

La prova riguarda due chat principali su questa macchina e un CODEX_HOME isolato, non un trasferimento SSH tra due dispositivi reali. Non verifica ancora una nuova risposta del modello, equivalenza semantica completa del contenuto, allegati, titoli personalizzati, pin, sezioni, progetti Codex, goal, strumenti dinamici, dipendenze fra chat o aggiornamento di una chat già caricata nel daemon della destinazione.

È stato osservato anche un JSONL di un'altra chat non interamente leggibile da un parser JSON standard. Oaclis deve distinguere file incompleti o non validi e rifiutare una sincronizzazione non verificabile; non deve correggerli silenziosamente.

## Fonte ufficiale e divergenza locale

La [documentazione ufficiale dell'app-server — Threads](https://learn.chatgpt.com/docs/app-server#threads) descrive thread/list, thread/read e thread/resume. Il testo consultato presenta limitazioni sulla ripresa delle chat paginate; la prova locale ha invece ripreso il campione paginato e ricostruito la cronologia. Per il supporto a Oaclis vale l'esito verificato sull'installazione target, con una prova di compatibilità da ripetere quando cambiano formato o versione.
