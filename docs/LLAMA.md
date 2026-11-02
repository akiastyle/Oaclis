# Collegamento llama.cpp

## Primo passo implementato

`POST /api/llama/models` sul master interroga il servizio esistente attraverso
il tunnel SSH AI-LAB. Restituisce soltanto motore, ID dei modelli e indicazione
del modello caricato. Non avvia inferenze, carica modelli o modifica chat.

La chiave viene letta dal file privato AI-LAB del master. Non passa al browser,
al relay o ai log. Il client rifiuta redirect e limita durata e dimensione
della risposta. Il catalogo non viene interrogato dal polling generale.

Prerequisito attuale: tunnel AI-LAB gia installato e attivo su loopback.
Questo passo non modifica installazione, servizi o configurazione del Jetson.
Un'installazione Oaclis senza AI-LAB continua a usare Codex normalmente;
una richiesta esplicita al catalogo llama.cpp segnala il collegamento assente.

## Integrazione successiva

- La creazione della chat scegliera motore e modello, lasciando Codex come
  default per tutte le chat esistenti.
- Le chat llama.cpp avranno identita e storico propri, senza creare falsi
  archivi Codex e senza convertire le chat esistenti. La persistenza passera
  dal proprietario Rust dello stato, senza introdurre scrittori indipendenti.
- Desktop e mobile riuseranno interfaccia, voce e autorizzazioni esistenti;
  il master inoltrera soltanto le richieste delle chat autorizzate.
- Lo streaming, l'annullamento e la coda devono essere collegati prima di
  rendere disponibile il nuovo motore nella UI. Il servizio attuale ammette
  una sola richiesta alla volta: non cambiare o scaricare modelli occupati
  da altre applicazioni.
- La parita con Codex nell'esecuzione di comandi e nella modifica dei file
  richiede una scelta esplicita dell'agente. Questo collegamento non concede
  al modello accesso alla shell o ai progetti.

Protocollo verificato sul servizio installato e nella
[documentazione llama.cpp](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md).
