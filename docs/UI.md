# UI locale

Avvio: `cargo run -- serve`, poi http://localhost:23232.

La mappa occupa la finestra; cliccando un dispositivo si apre la sua lista di chat Codex, escludendo i subagent. Titolo e UUID si aggiornano senza modificare Codex. La matita personalizza label, icona caricata e colore. Il colore del dispositivo è indipendente da quello della chat e del collegamento.

Una chat selezionata o trascinata apre la proposta. Tre icone indicano chat, cartelle e app/plugin/configurazioni: attive alla prima apertura, disattivabili singolarmente, con scelte persistenti per dispositivo sorgente e chat. Nessun percorso da compilare. Copia, sincronizzazione monodirezionale e bidirezionale restano scelte esplicite; Avvia esegue solo le categorie selezionate.

La selezione avvia l’analisi asincrona (Python 3.11+ e Codex disponibili sul dispositivo sorgente). Codex riceve evidenze dei progetti e il contesto recente della conversazione in una sessione effimera, senza riprendere o scrivere nella chat originale. Mantiene modello e provider configurati in Codex, senza caricare hook, MCP o impostazioni di permesso della configurazione utente. Il caricamento blocca Avvia; un errore permette di riprovare, senza una falsa percentuale. Analisi e selezioni sono salvate nel database locale `~/.config/oaclis/appearance.sqlite`. Cambiamenti nella chat o nei manifest delle dipendenze invalidano il risultato. Requisiti non risolti impediscono il completamento dell’analisi.

I percorsi rilevati vengono associati alla home del destinatario quando sono sotto la home sorgente; i percorsi di sistema mantengono la loro posizione. I file passano nei moduli Rust esistenti con rsync/SSH, confronti per contenuto e protezione dei conflitti. Cache, build, ambienti Python, Git e file riservati non sono replicati. I permessi dei file di progetto vengono mantenuti. Le configurazioni delle app vengono preparate nel verso scelto; il confronto delle cartelle segue la direzione della sincronizzazione.

App/plugin: rilevamento dai manifest Rust, Node, Python, PHP/Composer, Ruby, Go e Docker; preparazione dei pacchetti mancanti con gli strumenti Linux e installazione delle dipendenze sul destinatario. Le skill locali vengono copiate; i plugin Codex rilevati vengono installati da un marketplace locale trasferito via LAN, senza Git. I siti Apache/Nginx attivi associati al progetto e i file `.htaccess` vengono rilevati e preparati, con verifica della configurazione prima dell’attivazione. Le nuove configurazioni di server sono gestite con il prefisso `oaclis-`; quelle già gestite possono aggiornarsi solo se il destinatario corrisponde al checkpoint precedente. Le modifiche locali in conflitto vengono conservate. Moduli Apache riconosciuti: rewrite, ssl, proxy, proxy_http, headers, expires.

Temi chiaro/scuro, mappa spostabile anche con frecce, ricerca e avanzamento rsync. Layout e tema salvati nel browser. Controllo SSH ogni 15 secondi a pagina aperta; avviso sonoro opzionale al ritorno online. Nessuna esecuzione automatica al collegamento.

Server Rust su 127.0.0.1:23232 con controllo Host, token API e CSP. HTML/CSS/JavaScript locali, senza CDN. Dopo un riavvio, un token scaduto ricarica la pagina; le operazioni POST non vengono ripetute automaticamente.

Limiti: l’installazione automatica copre i manifest e i pacchetti riconosciuti, non un installatore generico di applicazioni arbitrarie. Credenziali personali, certificati privati e ambienti già compilati non vengono copiati. L’installazione di pacchetti o dipendenze può usare i repository del relativo gestore; i trasferimenti tra dispositivi restano in LAN. Abbinamento LAN HTTPS e regole automatiche non sono collegati. Nessuna cancellazione degli originali o annullamento rsync; attività in memoria. Una chat deve essere a riposo durante la copia. Una preparazione interrotta può lasciare pacchetti o file già trasferiti; gli errori vengono riportati e i dati originali conservati.

## Variabili CSS

I valori condivisi sono in `:root` in `ui/style.css`: colori, famiglia e scala dei caratteri, pesi e interlinee, spaziature, dimensioni, bordi, raggi, opacità, ombre e tempi degli effetti. Il tema scuro sovrascrive la palette. `--radius-control` regola i controlli, `--radius-panel` i pannelli e i dispositivi, `--shadow-surface` le superfici (attualmente `none`). Le coordinate calcolate dei dispositivi e il breakpoint mobile restano nella logica e nelle media query.

`--radius-icon` è condiviso dai pulsanti icona (inclusa modifica), dai moduli e dai tipi di sincronizzazione. Impostarlo a `50%` li rende circolari; input e pannelli mantengono i propri raggi.

## Palette degli accenti

Il selettore in basso sceglie tutte le 21 palette da https://venngage.com/blog/blue-color-palettes/ e conserva la scelta nel browser, separata da dark/light. `--color-sync`, `--color-send` e `--color-module` controllano gli accenti delle icone; `--radius-icon` ne controlla la forma. Pagina, toolbar e pannelli mantengono la palette neutra. Client e righe chat usano colori automatici dalla palette, assegnati in modo pseudo-casuale stabile per identificativo. I colori personalizzati nel database hanno precedenza; cambiando palette cambiano solo quelli automatici.

L’analisi esclude i percorsi dei dischi Windows `/mnt/c`, `/mnt/d`, ecc. prima di effettuare qualsiasi accesso al filesystem. Questi percorsi non diventano cartelle da sincronizzare.

La matita sul dispositivo apre un popover ancorato per nome e colore, chiuso dalla X. Il nome dei client aggiorna il registro dispositivi, senza cambiare ID o alias SSH; il nome del master resta una preferenza locale. Il pannello laterale non viene usato per questa modifica.

Box device: `--device-border-tint` al 100%, `--device-tint` all’80%. L’icona usa `--device-icon-color`, uguale a `--ink` del tema. La selezione conserva lo stesso sfondo; il testo usa `--ink` del tema. Anche il master usa un colore quando assegnato manualmente.

## Materiale vetro

Barre flottanti, box device, pannelli, popover, avvisi e porte usano lo stesso materiale: `--glass-opacity`, `--glass-device-opacity`, `--glass-blur`, `--glass-saturation`, `--glass-border`, `--glass-highlight` e `--shadow-surface`. Il vetro usa trasparenza reale e `backdrop-filter`; la base resta neutra secondo il tema, con la tinta scelta sui device. Nei browser senza backdrop-filter la superficie torna opaca.

Il selettore palette mostra cinque campioni colore, anche nel pulsante chiuso. Il menu usa colori del tema, si chiude con selezione, clic esterno o Escape e supporta frecce, Home/End ed Enter.

I device usano `--glass-device-blur: 3px` per lasciare riconoscibili i box sovrapposti; le altre superfici mantengono `--glass-blur: 18px`.

Gli strati flottanti usano `--z-toolbar`, `--z-panel`, `--z-activity` e `--z-notice`. I device restano sotto le barre di vetro; durante il drag `--z-device-drag` li porta davanti agli altri device, poi ripristina `--z-device`.

Tre porte sui client: bidirezionale a sinistra, invio e ricezione a destra. Ricevi apre le chat del client e usa il trasferimento monodirezionale verso il master. Ogni porta accetta solo chat dalla sorgente coerente con la direzione. Le frecce sono centrate nei pulsanti circolari.

Le associazioni chat/device della mappa sono conservate nella tabella `chat_links` del database SQLite di Oaclis, separate dall’esecuzione delle sincronizzazioni. La UI le legge da `/api/state`; `/api/links` permette di elencarle e salvarle. Le vecchie associazioni nel browser non vengono importate. Le linee passano sopra i box e mostrano la chat con un badge cliccabile. Sono visibili anche offline; non indicano che un trasferimento sia già completato.

Le tre ancore condividono gli stessi handler e gli effetti CSS: tutte pulsano durante il drag e hanno bagliore fisso al passaggio. Invia parte dal device della chat, Ricevi è sul destinatario. Le linee connettono le rispettive ancore: sync↔sync continua, send→receive tratteggiata, spessore `--connection-width: 3px`.

Il badge del collegamento è di 44px (`--link-badge-size`) e centrato a metà lunghezza della Bézier. Al passaggio o al focus mostra una sola matita. La matita del collegamento e quella della lista chat aprono lo stesso popover per nome, icona caricata e colore, con Salva, Ripristina e chiusura dalla X o Escape. Le modifiche sono salvate nelle preferenze chat del database e aggiornano entrambe le viste.

Aggiunta e configurazione del dispositivo si aprono in un popover vicino al comando; la connessione SSH è sotto Avanzato, chiuso inizialmente. Le impostazioni del trasferimento dopo un drop si aprono accanto all’ancora di destinazione, lasciando disponibile la lista chat. I pannelli condividono posizione entro il viewport, materiale vetro, X ed Escape. Il rilevamento/abbinamento automatico e i nuovi campi per server esterni non sono ancora implementati.

La matita del badge apre solo la personalizzazione. Il badge stesso apre la console Codex sulla destinazione dell’associazione, tramite `/api/terminal` autenticata e SSH. Richiede chat già presente, Codex e tmux sul device; non avvia trasferimenti. La sessione tmux resta attiva chiudendo il pannello, ed è riutilizzata alla riapertura. La console mostra uno snapshot testuale ogni 1,5 secondi e permette messaggi e tasti di navigazione; non è un emulatore terminale completo e non visualizza i colori ANSI.

Le console sono indipendenti per coppia chat/dispositivo e possono restare aperte contemporaneamente; riaprire la stessa coppia riporta in primo piano quella esistente. Il titolo permette di spostare la console. Per associazioni bidirezionali il clic mostra prima la scelta dei due dispositivi, marcati Locale/Remoto; per invii apre direttamente la destinazione. Ogni console ha polling e chiusura indipendenti.

Avanzato del dispositivo usa campi espliciti indirizzo/hostname, porta SSH, utente e percorso facoltativo della chiave, senza select di alias. Oaclis gestisce gli host in `~/.config/oaclis/ssh_config`, incluso dalla configurazione OpenSSH, e salva i valori in `ssh_settings.json`; le connessioni esistenti sono precompilate da `ssh -G`. Ogni dispositivo è raggiunto tramite i dati SSH, senza distinzione LAN/fuori LAN; il pallino indica la raggiungibilità. La verifica delle chiavi host resta attiva.

Il globo compare sui dispositivi diversi dal master e indica un altro dispositivo, indipendentemente dalla rete; non richiede una classificazione Fuori LAN.

Il globo usa una stima automatica: IP nella stessa sottorete di una delle interfacce locali = probabilmente in LAN, senza globo; altrimenti probabilmente remoto, con globo. In WSL considera anche IP e prefissi delle interfacce Windows. Per hostname o reti non disponibili la posizione resta incerta. È solo una convenzione visiva: non cambia SSH né le regole di trasferimento.

Personalizzazione unificata: chat, aggiunta e modifica dispositivo usano le stesse sezioni espandibili Colori, Icone e Carica icona. Colori mostra solo i cinque campioni della palette attiva; Icone mostra venti SVG e l’opzione iniziale, senza select. L’upload PNG/JPEG/WebP/GIF conserva limite 5 MB e conversione PNG 64×64. I dispositivi supportano anche immagini caricate; il master usa lo stesso editor, senza SSH né eliminazione.

Sincronizza nel menu chat contiene i toggle Chat, Percorsi e App. Le scelte sono preferenze globali della chat nella tabella `chat_settings`, aggiornano tutte le associazioni e le analisi e sono ereditate dai nuovi collegamenti. Sono applicate anche dai toggle nella proposta di trasferimento.

Elimina chat la nasconde nel catalogo Oaclis e rimuove tutte le associazioni e personalizzazioni locali. Elimina dispositivo rimuove dispositivo, regole, associazioni, aspetto e configurazione SSH gestita da Oaclis. Non cancella archivi Codex, cartelle trasferite, applicazioni, chiavi SSH o configurazioni SSH preesistenti; le console vengono chiuse senza terminare le sessioni Codex. Le chat nascoste restano nel database con `hidden=true`; non esiste ancora un comando UI per ripristinarle.

Regole comuni di personalizzazione: campioni compatti con 20 colori ricavati dalle palette (includendo la palette attiva), 24 icone predefinite più l’iniziale, upload mediante clic o trascinamento sulla stessa area. I dettagli dello stesso pannello condividono il nome HTML e aprono una sola sezione alla volta. La validazione e la conversione dell’immagine sono condivise fra upload da file e drop.

Il controllo SSH rimane periodico (15 secondi). Il catalogo chat non si aggiorna in background per device offline o dopo un errore; riprende alla riconnessione oppure con un aggiornamento esplicito. Il favicon non richiede il token API. Elimina è a tutta larghezza sotto Salva, con `--gap-delete: 30px`, rosso tenue e conferma nativa bloccante che descrive la cancellazione delle sole impostazioni Oaclis.

Movimento condiviso: `--duration-ui: 0.5s` e transizione `all` con `--ease-ui` su elementi e pseudo-elementi. Aperture dei pannelli e sezioni sfumano con un piccolo movimento verticale. Durante il trascinamento le transizioni geometriche sono disattivate per mantenere il controllo diretto. La preferenza di movimento ridotto disattiva transizioni e animazioni.

Toolbar unica in alto: i controlli palette, riordino mappa, suono e tema seguono aggiunta device e verifica collegamenti. Il pulsante palette usa un’icona; le anteprime restano nel menu, che si apre sotto il pulsante.

Logo balena: `ui/assets/oaclis-whale.svg` è il vettore ripulito e rifinito con sfumature, derivato dal riferimento JPG dopo conversione VTracer; i contorni non sono una riproduzione pixel per pixel. ViewBox quadrato di 548 unità con 4 unità di margine sul lato più lungo. Il PNG trasparente 1024×1024 è esportato dallo stesso SVG. Il logo precede Oaclis nella toolbar ed è usato come favicon SVG e icona touch PNG.

Marchio per ruolo: il file locale `~/.config/oaclis/role` accetta `master` o `client`, con default master per questa installazione. La corona sostituisce la balena nel marchio e nelle icone app del master; il client usa la balena. Il ruolo riguarda il marchio, non cambia il trasporto o le regole di sincronizzazione. Il lettering Oaclis usa Ubuntu Sans distribuito localmente, con licenza in `ui/assets/FONT-LICENSE.txt`, e due tonalità blu/crema adattate al tema. Le icone personalizzate dei box device restano quelle impostate dall’utente.

Passo successivo concordato: repository Git per distribuire Oaclis e installer/configurazione Windows, macOS, Linux, WSL e Docker. Git non viene usato per sincronizzare chat o progetti fra i device.
