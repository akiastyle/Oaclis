# Guida operativa di Oaclis

Questa guida descrive le attività disponibili e come avviarle.
Per installazione e rete: [INSTALLAZIONE.md](INSTALLAZIONE.md).

## Attività operative

| Attività realizzata | Motore o componente | Come si usa |
| --- | --- | --- |
| Conversazione, traduzione, riassunto e analisi del testo | Codex e llama.cpp | Aprire una chat, scegliere il modello, scrivere o dettare la richiesta e inviare. |
| Storico e ripresa della conversazione | Codex e llama.cpp | Riaprire la stessa chat dall'elenco. Ogni motore conserva il proprio archivio. |
| Lavoro sui file ed esecuzione di comandi | Codex | Scegliere la cartella del progetto e le autorizzazioni della chat, quindi chiedere l'operazione nella console. |
| Invio di immagini | Codex, con modello compatibile | Usare selezione file, trascinamento o incolla nella console. |
| Cambio del modello | Codex e llama.cpp | Premere Mod; in llama.cpp inviare poi `/model NOME-MODELLO` fra quelli elencati. |
| Accesso mobile alla chat | Codex e llama.cpp | Concedere la chat al telefono dal master e aprirla nella PWA. |
| Trasferimento e sincronizzazione degli archivi | Chat Codex | Configurare Chat/Percorsi/App e premere Play. |
| Avanzamento di script e calcoli | Endpoint statistiche Oaclis | Lo script pubblica testo e percentuale associati alla propria chat. |

## Configurare motore e modello

1. Nel desktop aprire **Motori AI** dalla barra principale, icona CPU.
2. Aprire la sezione del motore e selezionare **Abilitato**.
3. Impostare il modello predefinito, se desiderato, e le credenziali richieste.
4. Scegliere il motore predefinito per le nuove chat e premere **Salva**.

Codex usa la propria autenticazione. Per llama.cpp il collegamento locale
predefinito riusa il servizio Jetson attraverso il tunnel predisposto.
Sul master attuale è configurato `Qwen3-4B-Q4_K_M`.
Le chiavi già presenti si conservano lasciando vuoto il campo password.
Cambiare il motore predefinito non converte le chat esistenti.

Il pannello viene proposto su un'installazione vuota e resta accessibile dalla barra principale.

## Creare e aprire una chat

**Desktop:** aprire l'elenco delle chat del dispositivo e premere **Nuova chat** (+).
Scegliere nome, motore e modello. In **Avanzato**, impostare la cartella di lavoro
del progetto; le autorizzazioni della console sono nella sezione dedicata.
Premere **Crea chat**: la console si apre quando la creazione è completata.
Una nuova cartella di lavoro viene creata; per una cartella già esistente
viene chiesta conferma prima del riuso.

Per una chat esistente, usare **Apri console** nella riga della chat o nei
collegamenti della mappa. Per una chat condivisa scegliere il dispositivo
sul quale aprirla quando viene richiesto.

**PWA:** premere + nell'elenco o nella console, scegliere nome, motore e modello
e premere **Crea chat**. La creazione avviene sul computer associato;
la cartella viene ereditata dal contesto della chat da cui si crea.

## Usare e organizzare le console

| Azione | Comando | Risultato |
| --- | --- | --- |
| Inviare un messaggio | Pulsante freccia oppure Invio | Il testo viene inviato alla console selezionata. Shift+Invio inserisce un a capo. |
| Rispondere a una scelta numerata | 1, 2 o 3 | Il tasto viene inoltrato al programma nella console. |
| Aprire una domanda Codex in attesa | ⇧← | Invia Shift+freccia sinistra, come indicato da Codex con `shift+← to answer`. |
| Passare fra elementi previsti dal programma | Tab | Invia il tasto Tab alla console. |
| Richiamare i modelli | Mod | Invia `/model` seguito da Invio. |
| Interrompere il lavoro corrente | Esc | In Codex invia Escape; in llama.cpp interrompe la generazione. |
| Terminare la sessione selezionata | Ctrl+C | Ferma la sessione e chiude le sue viste. Lo storico della chat resta disponibile. |
| Chiudere soltanto la vista desktop | X | Chiude il pannello senza equivalere a Ctrl+C. |
| Staccare la console desktop | Icona finestra esterna nel titolo | Apre una finestra del browser separata. |
| Portare una console davanti alle altre | Clic nella console | Porta il pannello in primo piano e gli assegna il focus. |
| Affiancare sul desktop | Trascinamento verso bordi/angoli oppure Alt+frecce | Aggancia il pannello alla zona scelta. Posizione, dimensioni e modalità di apertura vengono ricordate. |
| Usare due console in PWA | Pulsante Affianca chat | Apre due pannelli sovrapposti. Selezionare quello superiore o inferiore per indirizzare i controlli condivisi. |

Per rinominare una chat o cambiarne colore, icona e autorizzazioni, usare la
matita nell'elenco chat e **Salva**. **Nascondi dalla lista**, **Elimina collegamento**
e **Elimina definitivamente** hanno effetti diversi: la cancellazione
permanente richiede conferma e non elimina la cartella del progetto.

## Collegare un computer

1. Installare e avviare Oaclis sui due computer.
2. Nel desktop premere **Aggiungi dispositivo** (+).
3. Selezionare il computer rilevato e completare l'approvazione richiesta
   sui due dispositivi. Usare **Aggiungi manualmente** se serve specificare
   il collegamento; per rete WSL e SSH seguire la guida di installazione.
4. Aprire l'elenco delle chat del computer dalla mappa.

La mappa consente di spostare dispositivi e icone senza avviare trasferimenti.

## Trasferire o sincronizzare una chat Codex

1. Dall'elenco del dispositivo sorgente, trascinare la chat sul canale
   appropriato del dispositivo di destinazione: invio oppure sincronizzazione.
2. Nei dettagli del collegamento scegliere separatamente **Chat**, **Percorsi**
   e **App**, verificando percorsi ed esclusioni.
3. Attendere inventari e confronto. Un conflitto o una preparazione mancante
   deve essere risolto prima dell'esecuzione.
4. Premere **Play** quando il collegamento è pronto e presenta differenze.
5. Seguire gli indicatori dei moduli fino all'esito finale. Aprire la console
   sulla destinazione per riprendere la chat trasferita.

L'invio monodirezionale conclude il lavoro dopo una copia riuscita.
La sincronizzazione bidirezionale continua a confrontare le copie, ma i
trasferimenti richiedono Play. L'origine viene conservata.
Il modulo App installa Node.js/npm e le dipendenze Node previste dal progetto,
verificando le versioni e registrando l’esito dell’installazione.

## Abilitare il telefono e scegliere le chat

1. Nel master aprire **Aggiungi dispositivo → Aggiungi mobile**.
2. Inquadrare il QR, creare la passkey sul telefono e approvare l'abbinamento
   dal master. Il QR di invito è temporaneo e monouso.
3. Per quel telefono aprire **Chat autorizzate** e selezionare le chat
   da rendere disponibili. La selezione viene salvata dal pannello.
4. Aprire la PWA sul telefono: quando serve autenticarsi viene richiesta
   direttamente la passkey. L'elenco contiene le chat concesse.

Per togliere una chat al telefono, deselezionarla nel master. La gestione e
la rimozione dei dispositivi restano nel desktop, non nella PWA.
Il telefono gestisce la propria interfaccia; l'esecuzione della chat avviene
sul computer collegato.

## Dettare e ascoltare

Usare **Chrome** e concedere il permesso del microfono. Il pulsante microfono
attiva la dettatura; il pulsante voce abilita la lettura delle risposte.
La guida ⓘ elenca comandi vocali e impostazioni, incluse attesa e invio automatico.

- Dettare il testo e usare «Invia» per spedirlo.
- Se la scrittura è in pausa dopo l'invio, dire «Riprendi» per tornare a dettare.
- Usare «Cambia» con il nome o il numero della chat per selezionarla.
- Spegnere il microfono con il pulsante o il comando previsto dalla guida.

La pausa di scrittura e lo spegnimento del microfono sono stati distinti.
Il numero della chat è condiviso fra desktop e PWA.

## Collegare le statistiche di un tool

L'endpoint è incluso di default con Oaclis; non si installa per ogni chat.
Lo script deve essere modificato nel progetto del tool per inviare lo stato:
Oaclis non ricava automaticamente la percentuale dall'output del calcolo.

1. Fornire alla chat che modifica il tool [TOOL-INTEGRATION.md](TOOL-INTEGRATION.md).
2. Usare l'ID reale della chat e un ID stabile diverso per ogni tool.
3. Lo script invia testo e percentuale a
   `POST http://127.0.0.1:23232/api/jobs/status` sul proprio dispositivo.
4. La percentuale compare nella chat. Clic o tap mostra il dettaglio per cinque secondi.
5. **Statistiche**, icona grafico nella barra desktop, raccoglie i tool di tutte
   le chat e dei computer abbinati; il pannello può essere staccato.

Un report al 100% produce l'avviso di completamento. Dopo `stop: true`
il tool deve interrompere gli aggiornamenti di quel lavoro.
Oaclis riceve lo stato: non avvia né dirige il calcolo dello script.

## Skill, aggiornamenti e controllo del dispositivo

| Attività | Dove e come |
| --- | --- |
| Vedere skill abilitate e aggiornamenti rilevati | Aprire **Skill**, icona libro nella barra desktop. Le righe sono raggruppate per dispositivo. |
| Abilitare gli aggiornamenti automatici supportati | Nel pannello Skill selezionare **Aggiorna automaticamente** per il dispositivo. La verifica all'avvio avviene anche senza questa opzione. |
| Aggiornare Oaclis su un computer | Usare l'indicatore di aggiornamento sulla card del dispositivo. L'operazione è esplicita e conserva chat e configurazioni; una repository modificata o divergente viene segnalata. |
| Riavviare Oaclis | Usare il comando di riavvio sulla card del computer e confermare. |
| Gestire lo standby | **Modalità energia** nella barra: Sempre acceso, Spegni monitor senza standby o Normale, secondo il supporto del sistema ospitante. |
| Aprire il desktop remoto | Usare il comando RustDesk del dispositivo quando il programma è installato e rilevato. Restano valide le autorizzazioni proprie di RustDesk. |
| Capire le operazioni in corso | Aprire **Log** nella barra. Il log operativo mostra dispositivi, chat e azioni; non è uno storico permanente. |

Per le attività ancora da completare consultare [TASKS.md](TASKS.md).
