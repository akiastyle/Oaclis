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

Relay state uses a separate state.lock for all readers/writers and long-poll waits.
Writes replace state.json atomically from a completed temporary file in the same
private directory. Never lock the renamed state file itself or rewrite it in place.
Expired mobile sessions return to passkey login; temporary errors retain the list
and retry, while an unavailable pairing shows a recovery message instead of hiding.

Console read errors retain the last screen and retry after one second. Pending
result IDs are reused after transient failures; input commands are never replayed.

Initialize a new relay with state.json containing {"invitations":[],"devices":[]}.
State reads clear PHP's file metadata cache and retry empty/failed reads briefly.
An unreadable state is never silently replaced with an empty device registry.

## Voce nella PWA

Il pulsante microfono nell'elenco chat e nella console avvia l'italiano parlato.
Il microfono verde indica la dettatura attiva; arancione lampeggiante indica
collegamento o elaborazione, giallo fisso indica la pausa di scrittura. Non ci
sono righe di stato, anteprime o pulsanti “?” sotto i comandi della console.
Le indicazioni di accessibilita restano disponibili ai lettori di schermo.
`help` apre a tutto schermo i comandi disponibili; `chiudi help` o la X lo chiudono.
`stop rec` mantiene il microfono in ascolto ma ignora dettatura e azioni di modifica;
`start rec` riprende la scrittura. In pausa restano utilizzabili i controlli di
registrazione, help e spegnimento. Un clic sul microfono acceso spegne tutto;
il clic successivo riattiva la dettatura. Scelte ambigue e conferme esplicite
usano pannelli a tutto schermo solo quando necessari.

- Dettatura: parlare nel messaggio o nel filtro selezionato. `scrivi invia` detta
  la parola riservata; una frase isolata uguale a un comando esegue quel comando.
- `scrivi messaggio`, `a capo`, `annulla dettatura`, `invia`.
- `svuota messaggio` richiede `conferma svuotamento` entro 10 secondi;
  `annulla operazione` interrompe. Una modifica della bozza annulla la conferma.
- `elenco chat`, `indietro`, `apri chat Santi`, `cerca chat Santi`,
  `seleziona chat numero tre`. I numeri corrispondono alla lista filtrata visibile
  (oppure all'ultima lista mostrata dalla console); supporto da uno a venti.
- Per nomi ambigui, scegliere un'opzione a schermo o pronunciare `scegli due`;
  `annulla selezione` interrompe, la scelta scade dopo 15 secondi.
- `scorri su`, `scorri giù`, `aiuto vocale`, `ferma ascolto` o `spegni microfono`.

Un segmento esegue una sola azione: i comandi non vengono cercati dentro frasi
libere. `comando teletrasporto` segnala un errore senza scrivere nella bozza.
La navigazione conserva le bozze per chat nella memoria della pagina; il reload,
la chiusura della pagina e il logout non le conservano. Nessun audio o risultato
vocale viene salvato nei log; solo un invio esplicito usa la console esistente.

Il riconoscimento dipende dal browser e puo usare servizi audio in rete.
Non e' garantito offline, in background o a schermo bloccato. Dopo il background
serve un nuovo tocco sul microfono; i retry dopo interruzioni sono limitati a tre.
Non viene aperto uno stream getUserMedia aggiuntivo. La sintesi vocale non e'
compresa in questa prima versione.
Verifiche automatiche con SpeechRecognition simulato: `node tools/test_mobile_voice.cjs`
(test locale ignorato da Git). Il test fisico S24/PWA resta da eseguire sul telefono.

Su Android ogni sessione vocale riconosce una frase, con anteprima provvisoria e
riarmo dopo il risultato definitivo finche il toggle resta acceso. Questo evita
la promozione dei risultati parziali a finali nella modalita continua di Chromium.
Le ripetizioni intenzionali in frasi successive restano valide: non si deduplica
confrontando il testo. Sugli altri sistemi resta la modalita continua.

Il toggle resta richiesto dopo una frase, un invio e le normali pause di silenzio
su Android. Durante l'invio l'ascolto e' sospeso e riprende da solo al termine.
Gli errori effettivi conservano il limite di tre retry; il silenzio Android non
e' un errore. Lo spegnimento esplicito e il background annullano ogni riarmo.
Un microfono realmente spento non puo ricevere il comando per riaccendersi:
serve il pulsante.
