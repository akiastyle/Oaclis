# Oaclis · Spazio 3D

Bozza locale con dati statici, separata dalla mappa operativa. Nessuna lettura degli archivi, nessuna chiamata alle API Oaclis, nessuna console reale e nessuna modifica ai device.

Avvio dalla radice del repository:

```sh
node prototypes/device-space3d/server.cjs
```

Aprire **http://localhost:23234/**. Il server ascolta solo su `127.0.0.1`.

Servono Node.js e un browser moderno con WebGL 2 abilitato; non occorre installare pacchetti npm per avviare la bozza.

- Trascinare lo sfondo per ruotare; rotella/pinch per lo zoom; tasto destro o due dita per traslare.
- Cliccare un cubo o la sua etichetta per aprire il pannello; cliccare una sfera per i dettagli del canale. I canali sono accessibili anche come pulsanti nel pannello del device.
- Attivare il comando “Sposta i dispositivi” e trascinare un cubo sul piano: archi e sfere seguono la posizione.
- “Inquadra” ripristina la vista iniziale; “Vista dall’alto” mostra il piano.
- “Simula passaggio dati” riproduce un flusso dimostrativo finito; un secondo clic lo ferma. A riposo non esiste un ciclo di rendering continuo.

La scena contiene ROG, KJ, KDell e S24, quattro sfere per device e archi 3D. I cubi hanno bordi smussati, luce, ombre e la stessa icona sulle sei facce. Stati, nomi delle chat e numeri sono fittizi: non indicano lo stato corrente della rete.

## Asset e dipendenze

Tutto viene servito localmente, senza CDN o chiamate esterne:

- Three.js **0.180.0**, distribuzione ufficiale npm `https://registry.npmjs.org/three/-/three-0.180.0.tgz`; moduli utilizzati e licenza MIT in `vendor/`.
- Icone dal catalogo Lucide già incluso in `ui/icons.js`, con la sua licenza esistente. `move.svg` proviene da `https://github.com/lucide-icons/lucide/blob/1.66.0/icons/move.svg` e segue la licenza Lucide in `ui/assets/LUCIDE-LICENSE.txt`.
- Logo esistente `ui/assets/oaclis-particles.png`, servito in sola lettura dal server della bozza.
- Nessuna nuova immagine generata; cubi, sfere, corone e archi sono geometrie native della scena. Le icone sono texture SVG del catalogo esistente.

## Verifica

Test locale `prototypes/device-space3d/test.cjs`: WebGL reale in Chromium, sei facce e quattro porte per device, menu e richiamo, accesso da tastiera ai canali, drag sul piano, simulazione interrompibile, rendering fermo a riposo, desktop e viewport stretta. Il test non misura prestazioni su hardware remoto.

Con il server avviato, eseguire `node prototypes/device-space3d/test.cjs`. Il test usa l’installazione Playwright in `/root/google-images-local/node_modules/playwright` e Chromium in `/root/.cache/ms-playwright/chromium-1228/chrome-linux64/chrome`: questi percorsi sono specifici dell’ambiente di verifica e vanno adattati su un altro computer.

Il sistema visivo di produzione (`DESIGN.md`) non è stato modificato. La bozza usa i suoi controlli essenziali e le icone del progetto; materiali e luci 3D sono specifici di questa esplorazione.
