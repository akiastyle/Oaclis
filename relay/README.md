# Relay mobile

Sorgenti della pagina gia' pubblicata su https://www.krprojects.it/oaclis-auth-test/index.php.
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
