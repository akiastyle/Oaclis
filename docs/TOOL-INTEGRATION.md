# Istruzioni per integrare un tool con Oaclis

Documento da fornire alla chat/AI che gestisce il progetto del tool.
Implementa soltanto l'invio dello stato: non modificare Oaclis, la sua UI,
le autorizzazioni o il funzionamento del calcolo.

## Dove inviare

```text
POST http://127.0.0.1:23232/api/jobs/status
Content-Type: application/json
```

Oaclis deve essere attivo sullo stesso device e raggiungibile dallo script.
`127.0.0.1` deve riferirsi all'ambiente dove gira Oaclis: un container, WSL
o un altro computer possono avere un loopback diverso. Se non e raggiungibile,
segnalalo e concorda l'indirizzo corretto; non aprire porte, modificare firewall
o creare tunnel/servizi di tua iniziativa.

Non inviare dati al sito pubblico o al relay mobile. Il master raccoglie
quelli dei device abbinati tramite il collegamento Oaclis gia esistente.
Questa API locale non richiede chiavi dei modelli AI, passkey o credenziali
FTP: non leggerle e non inserirle nei report.

## Primo collegamento: leggi il contratto

All'inizio invia `{}` allo stesso endpoint:

```sh
curl --max-time 2 --fail --silent --show-error \
  -X POST http://127.0.0.1:23232/api/jobs/status \
  -H 'Content-Type: application/json' \
  --data-binary '{}'
```

La risposta descrive protocollo, campi, limiti, esempio e frequenza degli
aggiornamenti. Non registra un tool e non modifica gli stati esistenti.
Leggi questo contratto una volta quando configuri il collegamento;
le richieste successive inviano lo stato completo. Non servono nuove
chiamate di configurazione per ogni aggiornamento.

## Cosa inviare

Solo questi quattro campi; non aggiungerne altri:

```json
{
  "chat_id": "CHAT_ID_REALE",
  "id": "spaaic-traduzioni",
  "text": "Rumeno: 3365 completate, 1635 rimanenti",
  "progress": 67.3
}
```

| Campo | Significato |
|---|---|
| `chat_id` | UUID/ID reale della chat associata al progetto in Oaclis. Non il titolo. |
| `id` | Identificativo stabile e leggibile del tool, per esempio `spaaic-traduzioni` o `vesuvio-calcolo`. |
| `text` | Testo libero che il tool vuole mostrare nel dettaglio. |
| `progress` | Percentuale numerica fra 0 e 100, oppure `null` se non e quantificabile. |

La chat/AI che modifica il tool deve usare il proprio ID reale di sessione,
lo stesso che identifica la chat e permette di riprenderla con `codex resume`.
Verificalo dal contesto disponibile e scrivilo direttamente nella configurazione
dell'invio del tool. Non richiedere al proprietario di passarlo manualmente
quando e gia disponibile. Non sono necessarie nuove variabili d'ambiente
fornite da Oaclis. Se l'ID non e verificabile, chiedilo senza inventarlo.

Assegna un `id` stabile e diverso a ogni tool della stessa chat: per esempio
`traduzioni-rumeno` e `calcolo-vesuvio`. I loro aggiornamenti restano separati.
`CHAT_ID_REALE` negli esempi e un segnaposto da sostituire.

Vincoli:

- `chat_id` e `id`: 1–160 caratteri, solo lettere ASCII, numeri, `-` e `_`.
- `text`: massimo 2000 caratteri; sono consentiti testo Unicode, a capo e tab.
  Niente HTML o Markdown da interpretare: Oaclis mostra testo semplice.
- `progress`: numero JSON, non una stringa e senza il simbolo `%`.
- Non inviare `NaN`, infinito, valori negativi o superiori a 100.
- Non inviare password, token, chiavi, dati personali non necessari o log completi.

Lo stato e identificato da device + chat + tool. Mantieni gli stessi ID
per tutti gli aggiornamenti e le successive esecuzioni dello stesso tool.
Ogni invio sostituisce il precedente: invia sempre lo stato completo,
non frammenti da accumulare. Un nuovo ID a ogni aggiornamento e sbagliato.

## Quando inviare

1. All'avvio del lavoro: descrizione iniziale e avanzamento reale.
2. Durante il lavoro: quando cambia l'avanzamento o il testo, al massimo
   una volta al secondo, accorpando gli aggiornamenti intermedi.
3. Ogni 15 secondi anche se nulla cambia, ripetendo lo stato corrente.
   Il segnale deve continuare anche durante calcoli o attese lunghe.
4. Alla conclusione riuscita: `progress: 100` e un breve riepilogo in `text`.
5. In caso di attesa, errore o interruzione: spiegarlo in `text`, mantenendo
   l'ultima percentuale affidabile oppure `null`. Non dichiarare 100%.

Il testo lo organizza il tool. Per le traduzioni puo contenere lingua,
completate e rimanenti; per un calcolo fase e scenario. Oaclis non calcola
contatori, non interpreta errori e non analizza l'output della console.
Se la percentuale non e affidabile usa `null`, senza stimarla arbitrariamente.

## Come integrare senza rallentare il lavoro

- Riusa il linguaggio e il client HTTP gia presenti nel progetto.
- Aggiungi una piccola funzione di invio e richiamala nei punti del ciclo
  che conoscono gia l'avanzamento. Riusa l'event loop/worker disponibile
  per il segnale periodico durante operazioni bloccanti.
- Non avviare processi, SSH o `curl` per ogni singolo elemento elaborato.
- Riusa la connessione quando possibile; una sola richiesta in corso e solo
  l'ultimo aggiornamento in attesa. Nessuna coda illimitata.
- Imposta un timeout breve, indicativamente 2 secondi. Oaclis spento,
  irraggiungibile o una risposta di errore non devono fermare il tool.
- Non attendere indefinitamente e non fare tentativi serrati. Riprova al
  successivo aggiornamento/segnale periodico. Limita i messaggi di errore ripetuti.
- Se ci sono piu worker, un solo coordinatore pubblica lo stato aggregato
  del tool: evita che si sovrascrivano a vicenda.
- Non introdurre database, file di stato, nuovi servizi o dipendenze pesanti
  per questo collegamento. Non cambiare la logica del lavoro.

## Risposta e verifica minima

Una richiesta accettata restituisce HTTP 200:

```json
{"ok":true}
```

Per una singola prova manuale, dopo avere sostituito l'ID reale della chat:

```sh
curl --max-time 2 --fail --silent --show-error \
  -X POST http://127.0.0.1:23232/api/jobs/status \
  -H 'Content-Type: application/json' \
  --data-binary '{"chat_id":"CHAT_ID_REALE","id":"spaaic-traduzioni","text":"Collegamento attivo","progress":null}'
```

Questo comando e una prova, non un comando da eseguire per ogni elemento.
Nel codice usa il serializzatore JSON del linguaggio, senza concatenare
manualmente testo o comandi shell.

Verifica soltanto che:

- la percentuale compaia nella chat corretta e il tap/click mostri `text`;
- il dettaglio si richiuda dopo 5 secondi e il tool sia nel pannello Lavori;
- un successivo invio aggiorni la stessa riga, senza duplicarla;
- un endpoint irraggiungibile non interrompa il lavoro.

Non riavviare servizi o processi attivi per fare queste prove senza una
richiesta del proprietario. Non modificare altri progetti.

## Cosa fa Oaclis

Gli stati restano in memoria: dopo un riavvio i tool li ripopolano con i
successivi invii. Senza aggiornamenti un lavoro locale incompleto diventa
grigio dopo 90 secondi; il 100% resta concluso. Non servono chiamate di lettura
per pubblicare lo stato: `POST /api/jobs/list` e destinato alla visualizzazione.

La UI mostra soltanto percentuale e testo ricevuti. Il monitoraggio non avvia,
arresta o sincronizza il tool e non fornisce comandi alle chat AI.
