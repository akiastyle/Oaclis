# Oaclis — specifica di progetto

## Scopo

Continuare il lavoro su una chat Codex e sul relativo progetto passando da un dispositivo a un altro, con pochi gesti e senza riscrivere credenziali, percorsi o comandi.

L'utente lavora su un dispositivo alla volta. La sincronizzazione serve al lavoro delocalizzato; non è richiesta la collaborazione simultanea sulla stessa chat.

## Ambiente concordato

- Quattro ambienti Ubuntu: questo WSL Windows, un secondo portatile con WSL, un Mac e un Jetson.
- Codex 0.162.0 su tutti i dispositivi, versione dichiarata dall'utente come ultima disponibile.
- SSH già configurato sulle macchine.
- Comunicazione esclusivamente nella stessa LAN.
- Tool scritto in Rust; tecnologia della GUI ancora da scegliere.
- Nessun cloud, servizio esterno, account remoto o passaggio attraverso Git per sincronizzare chat e file.
- Risorse dell'interfaccia disponibili localmente, senza CDN o font remoti a runtime.

Questi dati provengono dall'utente e costituiscono la base del progetto.

## Dispositivi e regole

Questo WSL è il master: definisce e modifica le regole, conserva le informazioni di collegamento SSH e distribuisce la configurazione ai dispositivi coinvolti.

Il master non è necessario durante un trasferimento o una sincronizzazione. Ogni dispositivo conserva le regole ricevute e può eseguirle direttamente con un altro dispositivo quando entrambi sono nella LAN e Oaclis è attivo.

Una regola identifica:

- Chat coinvolte e dispositivi interessati.
- Operazione: sincronizzazione oppure trasferimento.
- Cartelle associate e percorsi corrispondenti sui diversi dispositivi.
- File inclusi ed esclusi.
- Esecuzione automatica oppure previa approvazione.

Le regole possono essere cambiate. La gestione delle versioni e la propagazione delle modifiche a dispositivi temporaneamente offline restano da definire.

## Operazioni distinte

### Sincronizzazione

Mantiene aggiornate le chat e i file selezionati sui dispositivi previsti. Gli aggiornamenti passano direttamente fra i dispositivi raggiungibili; quelli offline li ricevono in un incontro successivo nella LAN.

L'uso di un dispositivo alla volta non esclude copie divergenti, per esempio dopo lavoro offline: non sovrascrivere silenziosamente dati differenti. Il criterio di confronto e il flusso di risoluzione devono essere definiti e verificati prima dell'automazione.

### Trasferimento

Rende disponibili una chat e il relativo progetto su uno specifico dispositivo per proseguire il lavoro. Non crea automaticamente una regola di sincronizzazione permanente.

La copia sull'origine rimane. Dopo aver verificato il funzionamento sulla destinazione, l'utente può cancellarla con un'azione separata ed esplicita. La cancellazione non è automatica; il suo perimetro, soprattutto per progetti condivisi fra più chat, deve essere definito prima di implementarla.

## Primo avvio e abbinamento

I controlli e i comandi provati durante la configurazione reale sono registrati in [docs/FIRST-RUN-WSL.md](docs/FIRST-RUN-WSL.md). Serviranno al wizard per rilevare i requisiti mancanti, proporre e attivare le azioni di configurazione sul dispositivo interessato, riducendo la necessità di digitare comandi. Distinguere passaggi verificati e ancora da provare, nonché privilegi Ubuntu e Windows.

1. Chiedere se collegare un altro dispositivo oppure configurarlo successivamente.
2. Controllare disponibilità del client SSH e stato del server SSH locale; distinguere questo controllo dalla raggiungibilità da un'altra macchina.
3. Rilevare altre istanze di Oaclis nella LAN e mostrarne nome e stato.
4. Consentire la selezione del dispositivo e richiedere accettazione su entrambe le macchine, con un codice di verifica comune.
5. Configurare l'accesso reciproco e verificarlo nelle due direzioni.

Scambiare chiavi pubbliche; mantenere ogni chiave privata sul dispositivo che la genera. Conservare host, utente, porta e riferimenti alle chiavi/configurazioni SSH. L'abbinamento non deve sostituire o danneggiare le autorizzazioni SSH già presenti.

Il rilevamento è service discovery, non sniffing. mDNS/DNS-SD è la proposta iniziale, da verificare in LAN e WSL; è prevista l'aggiunta tramite indirizzo quando il rilevamento non funziona. Il protocollo di abbinamento autenticato e l'autorizzazione SSH vanno ancora progettati: la sola presenza nella LAN non costituisce fiducia.

## Presenza, avvisi e approvazioni

Quando un dispositivo configurato diventa raggiungibile, segnalare la presenza sonoramente e avviare le operazioni automatiche previste dalle regole. Per le altre operazioni mostrare una proposta e attendere l'approvazione.

Il master offre queste funzioni quando è acceso; i dispositivi coinvolti devono poter operare anche quando è spento. Evitare avvisi ripetuti a ogni controllo di presenza.

Restano da definire il dispositivo che mostra la richiesta, chi approva, dove suona l'avviso e come evitare che due istanze avviino la stessa operazione.

## Chat, cartelle e percorsi

Proporre le cartelle del progetto partendo dalla directory di lavoro associata alla chat e dai percorsi usati nella conversazione e nei comandi.

La proposta deve essere confermata quando si configura la regola: un percorso citato potrebbe appartenere a un documento di riferimento, a credenziali o a file estranei al progetto. Non includerlo automaticamente.

Consentire la selezione visuale di cartelle, file ed esclusioni, nonché la corrispondenza fra percorsi diversi sui dispositivi. Non dedurre che due macchine abbiano lo stesso percorso o utente.

La prima prova su copie isolate di due chat principali, legacy e paginated, ha verificato rilevamento, ripresa e rimappatura della directory di lavoro tramite app-server locale. Risultati e limiti in [docs/CODEX-COMPATIBILITY.md](docs/CODEX-COMPATIBILITY.md). Restano da verificare gli allegati, i metadati aggiuntivi, le dipendenze fra chat e l'importazione su un dispositivo reale con chat già presenti. Non considerare la copia del solo testo una chat importata funzionante.

## Interfaccia e interazioni

Direzione concordata: essenziale, ordinata, pulita e moderna, con temi chiaro e scuro. Riferimenti estetici: https://tenzen.studio/ e https://elevenlabs.io/. Evitare decorazioni generiche, gradienti gratuiti, bagliori e statistiche non utili al compito.

Riferimento per la logica di oggetti e relazioni: `/var/lib/spaaic/public/struttura.html`, pubblicato da `/root/SPAAIC/_tools/lan/apache.conf` come `https://spaaic.local/struttura.html`. Riprendere la logica, non i colori né l'intero visualizzatore.

Impostazione iniziale:

- Barra superiore discreta con stato LAN, aggiunta dispositivo e scelta tema.
- Elenco laterale ricercabile di chat e progetti trascinabili.
- Mappa centrale con dispositivi spostabili e collegamenti selezionabili.
- Dettagli della selezione per cartelle, esclusioni e regole.
- Collegamenti che mostrano operazione, direzione e stato.

Trascinare una chat su un dispositivo propone trasferimento o sincronizzazione. Spostare un oggetto sulla mappa modifica solo il layout, senza avviare trasferimenti. L'esecuzione segue la regola e le eventuali approvazioni.

Consentire aggiunta, gestione e spostamento degli oggetti. Ridurre la digitazione attraverso selettori, conferme e impostazioni proposte. Offrire alternative tramite pulsanti e tastiera al drag & drop; gli stati devono essere leggibili anche senza affidarsi al colore.

Il layout finale e la tecnologia della GUI restano da validare con un prototipo. Rust è il requisito concordato; non è stata scelta una GUI web, nativa o ibrida.

## Integrità dei dati

Prima di un trasferimento reale definire e verificare:

- Come ottenere una copia consistente di chat e file se Codex o altri programmi sono aperti.
- Come verificare completezza e integrità sulla destinazione prima di dichiarare il successo.
- Come riprendere o annullare un trasferimento interrotto senza danneggiare copie funzionanti.
- Come gestire file diversi, eliminazioni, collegamenti simbolici, permessi e file sensibili.
- Come mantenere coerenti percorsi di progetto e riferimenti della chat sul dispositivo di destinazione.

Non sovrascrivere interi database o configurazioni Codex della destinazione per importare una chat. Non sincronizzare implicitamente credenziali Codex, chiavi private SSH o tutta la home.

## Prova minima di successo

Su due dispositivi nella LAN:

1. Abbinamento accettato sulle due macchine.
2. Selezione di una chat di prova e conferma della cartella associata.
3. Trasferimento sulla destinazione conservando l'origine.
4. Chat visibile e riprendibile in Codex sulla destinazione; progetto presente nel percorso concordato.
5. Nuovi messaggi e modifiche ai file riportati sull'altro dispositivo tramite una regola.
6. Operazione funzionante con master spento.
7. Disconnessione e copie divergenti gestite senza perdita silenziosa di dati.
8. Nessuna comunicazione Oaclis verso servizi esterni e nessun uso di Git per il trasferimento.

## Metodo di sviluppo

Il primo nucleo Rust è organizzato nei moduli indipendenti `chat`, `paths` e `compare`, esportati da una libreria e provabili con una CLI in sola lettura. Conservare questa separazione: UI e SSH devono utilizzare il nucleo senza incorporarvi logiche di presentazione o trasporto. Implementazione e limiti in [docs/RUST-CORE.md](docs/RUST-CORE.md).

Questa specifica registra gli accordi, non promette fattibilità tecnica ancora non verificata. Aggiornarla dopo ogni decisione sostanziale; mantenere le questioni aperte visibili invece di inventare risposte.

Procedere per passaggi verificabili, iniziando dalla compatibilità delle chat Codex e dal trasferimento manuale. Aggiungere automazione soltanto dopo aver verificato il percorso completo. Applicare Ponytail per contenere codice e dipendenze e Impeccable per UI/UX, rispettando i requisiti qui concordati.

## Trasporto e controllo: decisioni confermate

- Oaclis gestisce rilevamento, avvio, approvazioni e automazione; nessun cron o timer systemd.
- Copie effettive tramite rsync su SSH, mantenendo controlli di consistenza e conflitto in Oaclis.
- Porta predefinita scelta dall'utente: **23232/TCP** su master e client, configurabile, per UI, abbinamento, stato e controlli operativi. Comunicazioni remote previste in HTTPS con verifica dei certificati durante l'abbinamento.
- SSH conserva le proprie porte; rsync usa SSH. Discovery mDNS separato, con annuncio della porta di controllo.
- Dispositivi aggiungibili e modificabili, senza indirizzi incorporati nel codice.

Primo trasporto Rust di singoli file verificato realmente nelle due direzioni. Implementazione, comandi e limiti: [docs/RSYNC-TRANSPORT.md](docs/RSYNC-TRANSPORT.md). Il servizio sulla porta 23232 non è ancora implementato o esposto.

## Primo motore di aggiornamento

Implementati aggiornamenti di singoli file e chat con scelta della direzione, checkpoint locale, backup, verifica delle impronte e rifiuto dei conflitti. Prove LAN su dati temporanei nelle due direzioni superate. Non ancora abilitati su chat aperte; coordinamento Codex, alberi di progetto e regole distribuite restano successivi. [docs/SYNC.md](docs/SYNC.md) descrive il comportamento verificato.

## Regole per cartelle

Implementata la gestione manuale di cartelle tramite regole JSON modificabili: dispositivo, percorsi, copia/sincronizzazione, direzione e selezione dei file. Il salvataggio non avvia operazioni automatiche. Verificati nuovi file remoti, conflitti, esclusioni e cancellazioni non propagate sulle due WSL. Salvata una regola QR3D sul master, senza eseguirla sul progetto reale. Dettagli in [docs/RULES.md](docs/RULES.md). Chat e cartella non sono ancora unite in una stessa regola; servizio, distribuzione ai peer e UI restano successivi.
