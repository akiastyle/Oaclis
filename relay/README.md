# Relay mobile

Sorgenti della pagina gia' pubblicata su https://www.krprojects.it/oaclis/.
Questo componente PHP gira sull'hosting; Oaclis continua a funzionare in Rust.

I tre file pubblici sono index.php, app.js e style.css. La libreria lbuchs/WebAuthn v2.2.0
(commit 20adb4a240c3997bd8cac7dc4dde38ab0bea0ed1, MIT), la configurazione, lo stato e le
sessioni restano nella cartella privata oaclis-auth-test-private, fuori da www.
composer.lock documenta la versione della libreria; sul server esistente e' caricata
come library/src. Non caricare configurazioni, chiavi o stato nella directory pubblica.

Il master genera inviti, approva e revoca gli accessi. Nel master la chiave di gestione del
relay resta in ~/.config/oaclis/mobile-relay.json; non e' replicata agli slave.
La mappa usa soltanto ID pubblico, nome e stato dei telefoni approvati. Nessuna ricerca
SSH, analisi di chat/app/percorsi o sincronizzazione viene avviata per un telefono.

Attivo significa che la pagina mobile autenticata sta comunicando: l'ultima presenza
scade dopo 30 secondi e il master aggiorna la propria memoria ogni 10 secondi.
Lo stato non prova che il telefono sia acceso o connesso a Internet con l'app chiusa.
Le letture della UI usano la memoria; le comunicazioni con l'hosting avvengono fuori
dai lock dello snapshot. Un errore di rete conserva le card ma le indica non attive.
Eliminare una card revoca l'accesso; per ricollegarsi serve un nuovo abbinamento.

La card del telefono apre la selezione delle chat. Nessuna e' autorizzata di default.
Il master riusa catalogo locale, collegamenti gia' salvati e nomi personalizzati;
non avvia ricerche SSH o nuove analisi per aprire la selezione. La scelta e' salvata
nel record del singolo telefono sul relay. Solo ID e titoli selezionati sono inviati.
La pagina mobile riceve solo le proprie chat dopo la verifica di codice, passkey,
sessione e abbinamento approvato; non puo' leggere o modificare le scelte di altri
telefoni. La lista si aggiorna ogni cinque secondi, senza trasferire archivi o console.

Abbinamento e passkey: /oaclis/auth. La card apre il QR del solo indirizzo di accesso.
.htaccess instrada auth; legacy.php mantiene compatibili i vecchi link ed endpoint.

Una chat autorizzata apre la console del master. Il relay accoda soltanto open/read/input
con scadenza breve e un comando in attesa per telefono; Rust riusa terminal/tmux.
Il master riceve un comando alla volta, senza aprire porte pubbliche. Le risposte
restano isolate per telefono e chat; revoca, logout e rimozione del permesso bloccano
l'accesso. Il relay conserva solo l'ultima schermata, non l'archivio della chat.

L'installazione mobile usa manifest.webmanifest, le icone PNG e sw.js nel solo scope
/oaclis/. Il service worker usa esclusivamente la rete: nessuna cache di chat,
risposte della console, autorizzazioni o credenziali.

Console responsiveness: Rust reuses an HTTPS connection pool instead of launching curl.
The master checks every 200 ms while a console request is recent, and every two
seconds otherwise. The phone asks for the result immediately; the relay waits up
to two seconds, releasing both session and state locks between checks. Revocation,
chat grants and job identity are checked again after each wait. Console refreshes
wait 200 ms after a completed read; hidden pages retain the slower 1.5-second wait.
Input waits for an ongoing read rather than being silently discarded.
Relay response durations are recorded in the existing orchestrator log, without keys.
