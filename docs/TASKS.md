# Attività da affrontare

- [ ] Gestione multilingua di Oaclis. Attualmente UI, wizard e messaggi dell’app sono in italiano. Centralizzare i testi e prevedere una scelta della lingua persistente; localizzare anche date, numeri e messaggi operativi. Definire in seguito le lingue da supportare e se la preferenza debba restare locale o essere condivisa. I dettagli degli errori restituiti da strumenti esterni vanno distinti dai messaggi dell’app. Task da progettare e implementare in una fase successiva.

- [ ] Permesso Linux per ‘Sempre attivo’ nel wizard: verificare l’autorizzazione al blocco standby e indicare device e utente quando manca. Caso rilevato su KJ: Oaclis gira come kimrusso; il gruppo sudo richiede password e non autorizza automaticamente systemd-inhibit. Prevedere una configurazione guidata del permesso specifico, senza avviare Oaclis come root per aggirare il problema.
