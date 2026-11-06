# Oaclis · Spazio 3D

Bozza locale con dati statici, separata dalla mappa operativa. Nessuna lettura degli archivi, nessuna chiamata alle API Oaclis, nessuna console reale e nessuna modifica ai device.

Avvio dalla radice del repository:

```sh
node prototypes/device-space3d/server.cjs
```

Aprire **http://localhost:23234/**. Il server ascolta solo su `127.0.0.1`.

Servono Node.js e un browser moderno con WebGL 2 abilitato; non occorre installare pacchetti npm per avviare la bozza.

- Trascinare lo sfondo per ruotare; rotella/pinch per lo zoom; tasto destro o due dita per traslare.
- Cliccare un cubo o la sua etichetta per aprire il pannello; cliccare un fantasmino per i dettagli del canale o una sfera sugli archi per il menu della chat condivisa. I canali sono accessibili anche come pulsanti nel pannello del device.
- Afferrare direttamente un cubo e trascinarlo nello spazio 3D, senza selezionare modalità. Usare la rotella mentre lo si tiene afferrato per cambiarne la profondità. Archi, fantasmini e sfere seguono la posizione.
- Rilasciare un cubo sollevato per farlo cadere: gravità, rimbalzi più accentuati e una breve deformazione lo riportano a riposo. I cubi sono solidi fra loro anche durante il trascinamento: si può appoggiarli e impilarli; togliendo un supporto, quelli sopra ricadono. Un clic breve sul cubo apre il menu. Con la tastiera, premere Spazio sull’etichetta di un device per sollevarlo e lasciarlo cadere.
- “Inquadra” ripristina la vista iniziale; “Vista dall’alto” mostra il piano.
- Il pulsante con il fulmine (“Anima i fantasmini”) attiva o ferma il galleggiamento e l’ondulazione del bordo dei fantasmini. L’animazione continua mentre la pagina è visibile, con aggiornamenti limitati a 30 fps; parte disattivata se il browser richiede movimento ridotto.
- “Simula passaggio dati” riproduce un flusso dimostrativo finito; un secondo clic lo ferma. Con l’animazione dei fantasmini disattivata e fisica, hover e flusso terminati, non esiste un ciclo di rendering continuo a riposo.

La scena contiene ROG, KJ, KDell e S24, quattro fantasmini 3D in stile Pac-Man per device e archi con sfere delle chat condivise. Le sfere sono opache, illuminate fisicamente, proiettano e ricevono ombre; la loro icona resta sulla superficie frontale rivolta alla camera. I fantasmini, in rosso, arancione, ciano e rosa vivaci, mantengono l’ordine Sincronizzazione, Invio, Ricezione, Console remota su un arco rivolto alla camera. Gli occhi seguono il puntatore rispetto alla posizione di ciascun fantasmino, anche sopra i menu; il passaggio del mouse aggiunge un breve movimento. I cubi hanno bordi smussati, luce, ombre e la stessa icona sulle sei facce; il piano non ha griglia. La fisica torna completamente a riposo dopo i rimbalzi. Stati, nomi delle chat e numeri sono fittizi: non indicano lo stato corrente della rete.

## Asset e dipendenze

Tutto viene servito localmente, senza CDN o chiamate esterne:

- Three.js **0.180.0**, distribuzione ufficiale npm `https://registry.npmjs.org/three/-/three-0.180.0.tgz`; moduli utilizzati e licenza MIT in `vendor/`.
- Icone dal catalogo Lucide già incluso in `ui/icons.js`, con la sua licenza esistente.
- Logo esistente `ui/assets/oaclis-particles.png`, servito in sola lettura dal server della bozza.
- Nessuna nuova immagine generata; cubi, fantasmini, sfere, corone e archi sono geometrie native della scena. Le icone sono texture SVG del catalogo esistente. La fisica locale in `physics.js` non aggiunge dipendenze; gestisce cubi verticali con ingombri aggiornati durante la deformazione, collisioni durante la caduta e trascinamento a piccoli passi per impedire attraversamenti.

## Verifica

Test locale `prototypes/device-space3d/test.cjs`: WebGL reale in Chromium, sei facce e quattro fantasmini per device, ordine sul lato della camera, sfere opache con ombre e icone frontali, ondulazione dei fantasmini, occhi che seguono il puntatore, pausa dell’animazione, assenza della griglia, trascinamento diretto 3D e profondità con rotella senza pulsanti di modalità, rimbalzo accentuato e ritorno a riposo, menu e richiamo, accesso da tastiera ai canali, drag sul piano, simulazione interrompibile, rendering fermo con animazione disattivata, desktop e viewport stretta. Il test non misura prestazioni su hardware remoto.

Eseguire `node prototypes/device-space3d/physics.test.mjs` per verificare senza browser gravità, rimbalzi decrescenti, contatto col piano e fra cubi, impilamento, rimozione del supporto, trascinamento senza attraversamenti, deformazione, sospensione durante il trascinamento e arresto completo.

Con il server avviato, eseguire `node prototypes/device-space3d/test.cjs`. Il test usa l’installazione Playwright in `/root/google-images-local/node_modules/playwright` e Chromium in `/root/.cache/ms-playwright/chromium-1228/chrome-linux64/chrome`: questi percorsi sono specifici dell’ambiente di verifica e vanno adattati su un altro computer.

Il sistema visivo di produzione (`DESIGN.md`) non è stato modificato. La bozza usa i suoi controlli essenziali e le icone del progetto; materiali e luci 3D sono specifici di questa esplorazione.
