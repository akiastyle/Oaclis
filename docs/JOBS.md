# Stato dei tool

Uno script invia solo quattro campi a
`POST http://127.0.0.1:23232/api/jobs/status` sul device dove lavora:

```json
{
  "chat_id": "UUID-DELLA-CHAT",
  "id": "spaaic-traduzioni",
  "text": "Rumeno: 3365 completate, 1635 rimanenti",
  "progress": 67.3
}
```

`chat_id` identifica la chat Oaclis. `id` identifica il tool all'interno della
chat: lo stesso ID su chat o device diversi non sovrascrive gli altri.
Entrambi accettano lettere, numeri, trattini e underscore, fino a 160 caratteri.
`text` contiene fino a 2000 caratteri, anche su piu righe. Il tool decide cosa
scrivere: Oaclis lo mostra come testo semplice, senza interpretarlo.
`progress` va da 0 a 100; `null` indica un avanzamento non quantificabile.

Per conoscere il contratto inviare `{}` allo stesso endpoint: restituisce
campi, vincoli, esempio e frequenza, senza registrare un lavoro. La chat che
configura il tool inserisce direttamente il proprio ID reale e un ID distinto
per ogni tool. Non serve passare manualmente nuove variabili d'ambiente.
Le istruzioni per le altre AI sono in [TOOL-INTEGRATION.md](TOOL-INTEGRATION.md).

Risposta a uno stato accettato: `{"ok":true}`. Ogni invio sostituisce lo stato precedente. I dati
restano in memoria, non su disco, e al riavvio vengono ripopolati dai tool.
Gli aggiornamenti non avviano, arrestano o sincronizzano lavori.

Inviare al massimo un aggiornamento al secondo e ripetere lo stato ogni
15 secondi anche senza variazioni. Impostare un timeout breve: un fallimento
nel monitoraggio non deve fermare il calcolo, si riprova al successivo invio.
Dopo 90 secondi senza aggiornamenti un lavoro locale incompleto diventa grigio;
100% resta concluso. La percentuale non viene stimata da Oaclis.
Dopo un'ora senza nuovi aggiornamenti, lo stato temporaneo viene rimosso
dalla memoria. Un nuovo report lo rende nuovamente disponibile; il tool
non viene interrotto.

## Visualizzazione

- La barra del titolo delle console desktop/mobile mostra le percentuali dei
  tool associati alla chat. Click o tap mostra soltanto `text` e richiude il
  dettaglio dopo 5 secondi. Gli aggiornamenti non riavviano questo timer.
- La nav del master apre Statistiche: una riga per tool, con icona della chat, testo e percentuale.
  Raccoglie anche i tool dei device abbinati. Si puo staccare tutto il pannello
  in una finestra separata, che alla chiusura riapre il pannello nel master.
- Il pannello resta sopra gli altri pannelli Oaclis; una finestra browser
  separata non puo essere forzata sopra tutte le applicazioni del sistema.

`POST /api/jobs/list` legge gli stati aggregati. I report remoti arrivano
attraverso lo scambio SSH gia autorizzato, senza nuovi cicli SSH o scritture
di telemetria. Un peer non aggiornato non pubblica tool; in assenza dello
scambio, i suoi lavori incompleti diventano grigi dopo 30 secondi. Revocare
l'abbinamento rimuove la cache al normale giro dello scambio.

Le console ricevono i dati con le risposte gia esistenti: nessun polling
aggiuntivo. Il relay mobile inoltra soltanto quelli della chat autorizzata
per la richiesta firmata. Il pannello globale legge la memoria ogni 2 secondi
soltanto quando e aperto e la pagina e visibile. Sono accettati fino a 256 tool
per device; il relay limita la telemetria allegata a ogni risposta a 64 KiB.

L'endpoint di invio e locale, come le altre API Oaclis: non va esposto su
Internet. Ogni script comunica con Oaclis sul proprio device; il master
raccoglie i report dei peer aggiornati attraverso il collegamento esistente.
Questa implementazione non modifica gli script dei progetti.

## Completamento

Quando `progress` arriva a 100, Oaclis fa lampeggiare la percentuale per
5 secondi, emette una sola notifica sonora e poi nasconde l'indicatore e
la riga dal monitor. Una nuova esecuzione riparte inviando un valore
inferiore a 100, anche `null` se non quantificabile.

La risposta al report finale e `{"ok":true,"stop":true}`. Il tool deve
interrompere il proprio invio periodico dopo questa conferma: niente
altri heartbeat o report dello stesso completamento. Oaclis ignora i
report duplicati al 100%, senza riattivare l'avviso. Gli script gia
collegati devono essere adeguati a questa regola.
