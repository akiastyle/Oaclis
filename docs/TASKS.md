# Attività da affrontare

- [ ] Gestione multilingua di Oaclis. Attualmente UI, wizard e messaggi dell’app sono in italiano. Centralizzare i testi e prevedere una scelta della lingua persistente; localizzare anche date, numeri e messaggi operativi. Definire in seguito le lingue da supportare e se la preferenza debba restare locale o essere condivisa. I dettagli degli errori restituiti da strumenti esterni vanno distinti dai messaggi dell’app. Task da progettare e implementare in una fase successiva.

- [ ] Completare nel wizard l'autorizzazione interattiva per ‘Sempre attivo’ quando sudo richiede una password. Il comando dedicato `bash run.sh --prepare-awake` prepara ora il permesso specifico; la prima password va inserita dal terminale del dispositivo. Contesto originale: verificare l’autorizzazione al blocco standby e indicare device e utente quando manca. Caso rilevato su KJ: Oaclis gira come kimrusso; il gruppo sudo richiede password e non autorizza automaticamente systemd-inhibit. Prevedere una configurazione guidata del permesso specifico, senza avviare Oaclis come root per aggirare il problema.

- [ ] Supporto futuro a Claude Code, il CLI di Anthropic. Integrazione non attiva: Oaclis continua a usare Codex CLI. Valutare catalogo delle sessioni, riapertura della console e sincronizzazione, mantenendo distinti gli archivi e le identità delle chat dei due strumenti. Nessuna installazione o attivazione automatica. Riferimento: https://code.claude.com/docs/en/cli-reference

- [ ] Controllo sguardo: completare e verificare la calibrazione e il focus automatico delle sole finestre Oaclis. Funzione esclusa per ora dalla presentazione del prodotto. Stato delle prove in [window-focus.md](window-focus.md).

- [ ] Includere la preparazione di RustDesk nel setup standard, riutilizzando il supporto esistente e le autorizzazioni del desktop del sistema ospitante.

- [ ] Rivedere e mantenere aggiornato l’installatore delle dipendenze dei progetti: ampliare i tipi supportati e verificare le versioni sulla destinazione.
