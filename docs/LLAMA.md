# Collegamento llama.cpp

## Configurazione

Il pannello Motori AI si riapre dalla barra principale e viene proposto su
un'installazione vuota. Abilitazione, motore predefinito, chiavi e modelli
sono gestiti da Rust. La scelta del motore compare nella creazione chat
desktop e PWA. Codex e llama.cpp sono operativi; Claude resta predisposto
nella configurazione, senza conversazioni abilitate.

Le chiavi sono conservate in `~/.config/oaclis/engines.json`, con permessi
0600, fuori dal repository. I campi password vuoti conservano le chiavi
precedenti. Browser e relay ricevono soltanto la loro presenza, mai i valori.
La configurazione privata non viene replicata ai device. Codex conserva la
propria autenticazione; llama.cpp riusa la chiave AI-LAB esistente se non ne
è stata inserita una specifica. Nessuna chiave è compilata nel binario.

Per usare llama.cpp, abilitarlo nel pannello Motori AI e scegliere un modello
nella creazione chat. L'elenco arriva dal server collegato; se manca un
modello predefinito, la selezione iniziale preferisce quello già caricato.
L'endpoint predefinito è `http://127.0.0.1:18085/v1`, attraverso il tunnel
AI-LAB già predisposto. Il servizio e il tunnel devono essere disponibili:
questo collegamento li riusa senza modificarne la configurazione.

`POST /api/llama/models` legge il catalogo, non avvia inferenze né carica
modelli. Non viene interrogato dal polling generale. Un collegamento assente
produce un errore esplicito; Codex continua a funzionare indipendentemente.

## Conversazioni

Le chat llama.cpp usano la console condivisa: invio, aggiornamento in
streaming, lettura vocale e autorizzazione mobile. Il workflow del microfono
rimane invariato. Le richieste llama.cpp di Oaclis sono serializzate; fermare
una sessione annulla anche una richiesta in attesa.

Esc interrompe la generazione. Ctrl+C termina la sessione e chiude le relative
viste. Riaprendo la chat si ritrova lo storico salvato. Il pulsante Mod mostra
i modelli disponibili; `/model NOME-MODELLO` cambia modello conservando lo storico.

Sono conversazioni testuali: il modello non ha accesso a shell, file o
strumenti. Gli allegati non supportati vengono rifiutati esplicitamente.
I permessi Codex non concedono strumenti al modello locale.

Gli archivi sono separati da Codex, in `~/.config/oaclis/llama-chats.json`.
Il proprietario Rust dello stato li gestisce in memoria e li salva atomicamente
con il registro di recupero già usato dall'applicazione. Creazione, rinomina
ed eliminazione non invocano la gestione degli archivi Codex.

La console mostra la coda recente dello storico. Al modello viene inviato il
contesto recente entro il limite del client; l'archivio completo resta salvato.
Una generazione interrotta viene distinta da una risposta completata.

Il catalogo mostra le chat di entrambi i motori con la numerazione condivisa.
Le chat locali si possono concedere ai telefoni con gli stessi permessi per
chat. Le richieste mobile seguono il canale firmato esistente; il master
verifica identità e autorizzazioni prima di inoltrarle.

L'analisi dell'ambiente Codex e il trasferimento dei suoi archivi non vengono
applicati alle conversazioni llama.cpp. Il trasferimento degli archivi locali
fra computer resta un passaggio successivo.

## Test sul Jetson

Verificati il 16 novembre 2026: catalogo di 14 modelli, inferenza reale con
`Qwen3-4B-Q4_K_M`, due messaggi con mantenimento del contesto e risposta SSE.

Test esplicito del collegamento, senza chat persistenti:

```bash
cargo test --lib llama::tests::installed_jetson_conversation -- --ignored --nocapture
```

Il test usa le credenziali private già configurate. Carica il modello di prova
sul servizio esistente e si ferma se trova un altro modello caricato.
Non è una prenotazione del server: eseguirlo quando gli altri tool non lo usano.
Il modello può restare caricato secondo le regole del server esistente.

Test del ciclo della chat, con archivio temporaneo isolato, persistenza,
riapertura e interruzione di uno streaming reale:

```bash
cargo test --lib local_chat::tests::installed_jetson_chat_lifecycle -- --ignored --nocapture
```

Questi test non partono con i test ordinari. La prova del ciclo elimina soltanto
il proprio archivio temporaneo e non modifica chat reali.

Protocollo: [documentazione del server llama.cpp](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md).
