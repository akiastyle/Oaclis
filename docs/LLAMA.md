# Collegamento llama.cpp

## Configurazione dei motori

Il pannello Motori AI si riapre dalla barra principale. Compare automaticamente
su un'installazione vuota e non ancora configurata, senza modificare gli script
di avvio. Abilitazione, motore predefinito e modelli vengono gestiti da Rust.
La stessa scelta del motore compare nella creazione chat desktop e PWA.

La configurazione privata e le eventuali nuove chiavi sono conservate in
`~/.config/oaclis/engines.json` con permessi 0600, fuori dal repository. I campi
password vuoti conservano le chiavi precedenti. Le risposte al browser e al
relay contengono soltanto la loro presenza, mai i valori. La configurazione
non viene replicata ai device. Codex continua a usare la propria autenticazione;
llama.cpp riusa la chiave AI-LAB esistente finche non viene inserita una chiave
specifica nel pannello. Nessuna chiave viene compilata nel binario.

Le richieste di elenco motori/modelli dalla PWA seguono lo stesso canale
firmato e gli stessi vincoli di associazione e chat autorizzata della console.
Non contengono credenziali e non aprono una sessione Codex.

Stato attuale: Codex e operativo. Claude e llama.cpp sono configurabili, ma
la creazione delle relative chat rimane esplicitamente disabilitata finche
non vengono integrati storico e gestione dei messaggi. Anche il backend
rifiuta queste creazioni prima di toccare gli archivi Codex.

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

- La scelta di motore e modello e presente; resta da attivare la creazione
  delle chat dei nuovi motori. Le chat esistenti rimangono Codex.
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
