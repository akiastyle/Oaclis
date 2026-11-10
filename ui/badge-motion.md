# Badge animato

`badge-motion.js` e `badge-motion.css` gestiscono soltanto la presentazione.
Nessun accesso a rete o DB e nessun avanzamento simulato. L'istanza vive sul
badge esistente e non viene ricreata quando cambiano posizione o progressi.

`new OaclisBadgeMotion(elemento, simboli)` riceve i tracciati SVG Chat, Percorsi
ed App. `update({phase, modules})` riceve:

- `phase`: `idle`, `checking`, `transferring`, `paused`.
- `modules`: chiavi `chat`, `paths`, `apps`, ciascuna con `changed` booleano
  e `progress` numerico fra 0 e 1, calcolati dal motore.

Solo `changed: true` mostra l'anello. Un modulo completato resta visibile
finché il motore conclude il trasferimento e comunica `idle`.
`paused` conserva i valori ricevuti. Il motore resta responsabile di salvare
l'avanzamento, verificare i dati e decidere quando riprendere.

Aggiornamenti identici non riavviano le animazioni. `destroy()` annulla i
callback pendenti quando il collegamento viene rimosso. I valori grafici
condivisi sono variabili CSS; il componente rispetta `prefers-reduced-motion`.

L'adapter in `app.js` legge `job.module_progress` o il risultato persistito
in `transfers`. Il contratto è predisposto: il motore attuale non produce
ancora questi progressi separati; non vengono ricavati dalla selezione dei
moduli e non sono inventati. Il loading del check usa gli stati già presenti.
