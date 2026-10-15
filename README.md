# Oaclis

Applicazione Rust per gestire copie e sincronizzazioni di chat Codex, cartelle e ambiente tramite SSH e rsync. Git distribuisce il programma; non trasporta le chat.

## Combinazioni di sistema e ambiente

Prima di procedere, scegliere dove verrà eseguito Oaclis: sul sistema direttamente, nella WSL oppure dentro un container. La presenza di Docker sul computer non implica che Oaclis debba essere installato in Docker.

| Sistema del computer | Ambiente scelto per Oaclis | Dove eseguire la preparazione | Script | Gestione del collegamento sulla 23232 |
| --- | --- | --- | --- | --- |
| Linux, compreso Jetson | Linux nativo | Terminale Linux | `bash run.sh` | Interfacce e firewall del dispositivo. |
| Linux | Container Linux con Docker Engine | Terminale dentro il container | `bash run.sh` | Configurare la rete del container; la rete host è un'opzione da verificare. |
| Linux | Container Linux con Docker Desktop o Docker in una VM | Terminale dentro il container | `bash run.sh` | Considerare anche la rete della VM; non assumere la stessa configurazione di Docker Engine diretto. |
| macOS | macOS nativo | Terminale macOS | `bash run.sh` | Interfacce e firewall di macOS; collaudo su Mac ancora da fare. |
| macOS | Container Linux con Docker Desktop o Docker in una VM | Terminale dentro il container | `bash run.sh` | Gestire il percorso container → VM → rete del Mac; procedura LAN ancora da definire. |
| Windows | Windows nativo | PowerShell | `.\run.ps1 -Check` | Solo controlli: esecuzione nativa di Oaclis non ancora supportata. |
| Windows | Distribuzione Linux in WSL | PowerShell con distribuzione scelta esplicitamente | `.\run.ps1 -Distro Ubuntu` | Windows prepara rete, standby e browser; richiama `run.sh` per installazione e servizio Linux. |
| Windows | Container Linux con Docker Desktop | Terminale dentro il container | `bash run.sh` | Gestire rete Docker e sistema ospitante; procedura LAN e avvio guidato da PowerShell ancora da definire. |
| Windows | Container Linux con Docker Engine dentro WSL | Terminale dentro il container | `bash run.sh` | Considerare entrambi i livelli: rete del container e rete WSL. Non basta preparare soltanto WSL. |
| Windows | Container Windows | PowerShell nel container | Nessun percorso funzionante attuale | Non supportato: non trattarlo come un container Linux. |

### Cosa cambia quando si sceglie Docker

Su Windows, Linux e macOS, il container Linux ha i propri strumenti, utente e cartelle: Git e i prerequisiti devono essere disponibili **dentro il container**, anche se sono già installati sul computer. Scaricare o rendere disponibile la repository nel container, poi eseguire lì `run.sh`. Il gestore dei pacchetti dipende dalla distribuzione del container, non dal sistema del computer: i comandi `apt` valgono solo per Ubuntu/Debian e derivate. Lo script attuale installa automaticamente solo su queste distribuzioni Linux.

Il percorso Docker dovrà inoltre verificare che Docker sia installato e attivo sull'host, predisporre l'accesso al container e scegliere volumi persistenti per configurazioni, chat e progetti. Queste operazioni **non sono ancora gestite dai due script**: `run.ps1` controlla soltanto la presenza del comando Docker, mentre `run.sh` segnala il container.

La porta Oaclis resta **23232** in tutte le combinazioni. La sua pubblicazione, l'accessibilità dalla LAN e il passaggio degli annunci di discovery sono verifiche distinte: pubblicare la porta non dimostra che il broadcast raggiunga gli altri dispositivi. La configurazione Docker non è ancora implementata o collaudata; non assumere che una soluzione provata su Docker Engine Linux funzioni anche su Docker Desktop Windows/macOS.

## Avvio della preparazione

Dopo aver scaricato la repository, il primo comando da eseguire dipende dall’ambiente. In modalità normale, `run.sh` prepara i prerequisiti e poi compila e installa automaticamente Oaclis in `~/.local/bin/oaclis`, senza un’altra conferma. `run.ps1` richiama lo stesso percorso solo quando si sceglie WSL. Non avviano la UI e non modificano rete o autorizzazioni SSH. Dopo l’installazione eseguono `oaclis setup`: cercano il master sulla 23232 e configurano la nuova installazione come client se ne trovano uno. Un ruolo già configurato resta conservato. Le installazioni delle dipendenze richiedono una conferma esplicita.

### macOS / Linux / WSL / container Docker

Nel terminale del sistema o del container, dalla cartella `Oaclis`:

```bash
bash run.sh
```

Per eseguire soltanto i controlli, senza installazioni, compilazione o scritture nei settings:

```bash
bash run.sh --check
```

Il controllo distingue Linux/macOS, segnala container e kernel WSL, verifica gli strumenti presenti e Rust. Le installazioni automatiche sono disponibili su Ubuntu/Debian con apt e su macOS con Homebrew già presente; gli strumenti Apple mancanti vengono segnalati. Se Codex non è rilevato, propone un’installazione opzionale di Codex 0.162.1. Verifica Node.js e npm; se mancanti o Node è incompatibile, propone Node.js 22/npm tramite NVM nella home, mostrando anche la modifica della configurazione shell. Ogni installazione richiede conferma. Il rifiuto o un errore nella preparazione di Codex non blocca Oaclis. Verifica anche l’autenticazione di Codex: se manca, propone l’accesso ChatGPT con `codex login --device-auth`, da completare personalmente nel browser anche su un altro computer. Se rimandato, segnala che analisi e chat potrebbero non funzionare. Non modifica llama.cpp. `--check` termina con codice 1 se mancano prerequisiti.

### Windows — PowerShell

Dalla cartella `Oaclis`:

```powershell
.\run.ps1 -Check
```

Lo script controlla gli strumenti Windows e mostra le distribuzioni WSL disponibili. **Windows nativo non è ancora supportato dal programma**: non viene presentato come un’installazione funzionante.

Solo se si sceglie di usare una distribuzione WSL già installata, indicarla esplicitamente:

```powershell
.\run.ps1 -Distro Ubuntu
```

Questo passaggio chiede conferma e poi esegue `run.sh` nella distribuzione scelta. Per i soli controlli dentro quella distribuzione:

```powershell
.\run.ps1 -Distro Ubuntu -Check
```

Se PowerShell impedisce l’esecuzione dello script, non modificare globalmente la policy: riportare l’errore prima di proseguire.

## Ricerca del master dopo l’installazione

Alla fine di `run.sh`, il binario installato esegue `oaclis setup`. Il master deve essere acceso e Oaclis deve essere avviato sul master. La ricerca usa gli annunci UDP **23232** e interroga `/peer/hello` via TCP **23232** sugli IP della tabella dei vicini. Su Linux usa `ip neigh`, già controllato dai prerequisiti: non serve installare `arp` o `net-tools`. Su macOS usa `arp`; in WSL considera anche i vicini della rete Windows.

Un IP o una porta aperta non bastano: solo una risposta firmata con identità coerente e ruolo `master` è considerata un master Oaclis. Se viene trovato un solo master, lo script salva il ruolo `client` e l’identità/nome/ultimo indirizzo del master in `~/.config/oaclis/settings.json`, con `authorized: false`. Il file `role` viene aggiornato per il runtime. Le altre impostazioni restano conservate.

Se vengono trovati più master, chiede quale scegliere. Se non ne trova, propone di riprovare, configurare esplicitamente questa macchina come master o rimandare. Senza terminale interattivo lascia il ruolo da configurare. Un’installazione nuova non si annuncia automaticamente come master.

Per ripetere soltanto questo passaggio, senza ricompilare:

```bash
~/.local/bin/oaclis setup
```

Il rilevamento e il ruolo non autorizzano SSH e non trasferiscono chat: la conferma reciproca del collegamento rimane un passaggio successivo. `--check` non esegue la ricerca né modifica il ruolo. Se la porta UDP 23232 è già occupata da Oaclis, arrestare quell’istanza prima di eseguire `setup`.

## Settings locali dello scenario

In modalità normale, gli script salvano lo scenario in `settings.json`:

- Linux, macOS, WSL e container Linux: `~/.config/oaclis/settings.json`.
- Launcher Windows: `%LOCALAPPDATA%\Oaclis\settings.json`.

Il file registra anche disponibilità nel PATH e percorsi di Node.js, npm e Codex, senza credenziali. Per l’installazione npm viene usato il metodo indicato nella [documentazione ufficiale Codex](https://learn.chatgpt.com/docs/codex/cli). Il file registra versione del formato, sistema rilevato, sistema ospitante quando noto, ambiente nativo/WSL/container, kernel WSL, eventuale distribuzione WSL e porta di controllo **23232**. Conserva le altre impostazioni già presenti. `--check` e `-Check` restano senza scritture.

`run.sh` salva dopo che Python 3 è disponibile. Da un container il sistema ospitante può essere sconosciuto: viene salvato come `null`, non indovinato. Se noto, dichiararlo prima dell'esecuzione, per esempio:

```bash
OACLIS_HOST_OS=windows bash run.sh
```

Sono ammessi `windows`, `macos`, `linux`. Con `run.ps1 -Distro Ubuntu`, Windows passa esplicitamente sistema ospitante e distribuzione allo script nella WSL. Le impostazioni del launcher Windows e quelle del runtime WSL sono file distinti.

Per scegliere una cartella diversa usare `OACLIS_SETTINGS_DIR`; nei container la cartella va conservata in un volume persistente. La creazione e il collegamento di quel volume restano da definire nel percorso Docker.

Questo passo salva soltanto lo scenario locale. Il comando `oaclis setup` legge questi settings, ne conserva lo scenario e aggiunge ruolo e ultimo master trovato. Il riuso degli indirizzi degli altri client e l’autorizzazione restano da completare. Non vengono salvate password o chiavi private nel file.

## Scaricare la repository prima della preparazione

Gli script fanno parte della repository: per scaricarli servono Git e i due passaggi seguenti.

## Primo passaggio: installare Git

Nel terminale Ubuntu/Debian, nella WSL Ubuntu o nel container Docker basato su Ubuntu/Debian, prima di scaricare Oaclis:

```bash
sudo apt update
sudo apt install -y git
```

Se sei già **root**, esegui gli stessi comandi senza `sudo`:

```bash
apt update
apt install -y git
```

## Secondo passaggio: scaricare Oaclis

Su Linux, macOS o WSL, usa lo stesso utente con cui lavori in Codex e scarica la repository nella sua home. Usa `sudo` per installare i pacchetti o per le operazioni amministrative, non per il clone o per avviare Oaclis.

Se sei entrato in una shell root con `sudo -i` o `sudo su`, torna al tuo utente e verifica il nome:

```bash
exit
whoami
```

Il comando `exit` serve solo se sei nella shell root. Poi scarica la repository nella home dell'utente; `git clone` crea automaticamente la cartella `Oaclis`:

```bash
cd ~
git clone https://github.com/akiastyle/Oaclis.git
cd Oaclis
```

Sul Jetson della prova l'utente è `kimrusso` e la cartella è `/home/kimrusso/Oaclis`. L'archivio Codex di root è distinto da quello di `kimrusso`.

Questo passaggio scarica il progetto; non installa né avvia ancora Oaclis.

## Collegamento fra due dispositivi

Il rilevamento tramite tabella dei vicini e annuncio TCP è stato verificato dal Jetson verso ROG. Il collegamento autorizzato fra tutti i dispositivi resta da collaudare.

Il flusso è lo stesso per master e client: avviare Oaclis su ciascuno, aprire `http://localhost:23232`, attendere che appaia l’altro dispositivo, confrontare il codice e accettare su entrambi. La ricerca è automatica, sulla **porta 23232**. Il pannello è accessibile anche da **+**. Una sola conferma lascia il collegamento in attesa; nessuna chat viene trasferita durante l’abbinamento.

Il rilevamento usa le interfacce IPv4 attive e i relativi broadcast. Su Linux nativo e Jetson non viene invocato Windows; macOS usa il proprio adattatore di rete. In WSL NAT si usa un ponte locale verso la rete Windows. La preparazione specifica compare solo quando il controllo rileva un inoltro mancante o errato: **Prepara collegamento** richiede la conferma amministrativa del sistema ospitante. Non viene richiesto questo passaggio sui dispositivi con rete già disponibile.

La scoperta non dipende dalla presenza di un server SSH. Se SSH manca, il dispositivo può comunque apparire e ricevere la conferma; l’accesso operativo viene completato quando SSH è pronto. Gli indirizzi vengono ricontrollati durante l’esecuzione; loopback e bridge spenti non sono destinazioni LAN.

In Docker, una rete bridge isolata non inoltra i broadcast della LAN. La prima configurazione da provare su un host Linux è la rete host, riservando la porta 23232 a una sola istanza e predisponendo separatamente SSH del container. Non è stato implementato un ponte automatico Docker o un installer Windows nativo. La discovery macOS è coperta da test con dati di interfaccia, ma non ancora collaudata su hardware Mac.

## Installazione e prima configurazione: Ubuntu / Linux / WSL

Eseguire i passaggi sulla macchina da aggiungere. I blocchi indicano il terminale da usare. In Ubuntu, se si è già root, `sudo` si può omettere. Usare lo stesso utente con cui si vogliono gestire le chat Codex: ogni utente ha un archivio distinto in `~/.codex`.

### 1. WSL, se non è già installato — PowerShell amministratore

```powershell
wsl --install -d Ubuntu
```

Riavviare Windows se richiesto, aprire Ubuntu e completare la creazione dell’utente. Per una WSL già presente, basta controllare e avviare la distribuzione:

```powershell
wsl --list --verbose
wsl -d Ubuntu
```

Sostituire `Ubuntu` con il nome mostrato dall’elenco se diverso. Per usare root, come nella prova ROG/KDell: `wsl -d Ubuntu -u root`.

### 2. Dipendenze — terminale Ubuntu

```bash
sudo apt update
sudo apt install -y git curl ca-certificates build-essential python3 openssh-client openssh-server rsync tmux
```

Verificare Rust prima di installarlo:

```bash
command -v cargo
cargo --version
rustc --version
```

Se assente o incompatibile, installare il toolchain stabile con [rustup ufficiale](https://rust-lang.org/tools/install/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/oaclis-rustup.sh
sh /tmp/oaclis-rustup.sh
. "$HOME/.cargo/env"
cargo --version
rustc --version
```

Su KDell è stata verificata anche l’alternativa `sudo apt install -y cargo build-essential`, con Rust/Cargo 1.93.1 dai repository Ubuntu. Su release Ubuntu precedenti i pacchetti possono essere troppo vecchi: preferire rustup. Servono edition 2024 e compatibilità con le dipendenze bloccate in `Cargo.lock`.

Per aprire le chat, Codex deve essere installato e autenticato sul dispositivo; `run.sh` può proporne l’installazione, e, se manca il login, propone l’accesso ChatGPT con `codex login --device-auth`; non copiare le credenziali da un altro computer. Per le console delle chat verificare:

```bash
command -v codex
codex --version
command -v tmux
```

### 3. Scaricare e installare Oaclis — terminale Ubuntu

```bash
cd ~
git clone https://github.com/akiastyle/Oaclis.git
cd Oaclis
bash scripts/install.sh client
```

Sul primo dispositivo, destinato a essere master, usare invece:

```bash
bash scripts/install.sh master
```

L’installer compila il binario in modalità release, lo installa in `~/.local/bin/oaclis` e crea `~/.config/oaclis/role` solo se non esiste. Non cambia impostazioni, chat o chiavi esistenti. Su una macchina già configurata non basta passare un altro ruolo per cambiarlo.

### Collegamento automatico: percorso da usare dopo l’installazione

Avviare SSH (passaggio 4 sotto), poi Oaclis **su entrambi i dispositivi**:

```bash
~/.local/bin/oaclis serve
```

Aprire `http://localhost:23232` su ciascuno e premere **+** nella toolbar: il pannello cerca gli altri Oaclis. Se il pannello rileva che la rete WSL NAT richiede preparazione, premere **Prepara collegamento** e accettare la richiesta amministrativa Windows su ciascun computer. Vengono configurati soltanto gli inoltri SSH `2222 → 22`, controllo `23232 → 23232` e le regole firewall Oaclis TCP/UDP limitate alla sottorete locale. Il ponte di discovery Windows parte con Oaclis e termina quando l’app non risponde più; non si aggiungono servizi o attività pianificate.

Quando compare l’altro dispositivo, confrontare il codice mostrato nelle due UI e premere **Il codice coincide · Accetta su entrambe**. Una sola accettazione lascia il collegamento in attesa. Dopo la seconda, Oaclis autorizza le chiavi pubbliche dedicate, salva l’identità host SSH e aggiunge il dispositivo alla mappa; il controllo SSH della mappa verifica poi l’accesso operativo. Le chiavi private restano sul dispositivo che le ha generate. Non vengono copiati chat, file o credenziali durante l’abbinamento.

Gli annunci LAN sono firmati con Ed25519 e scadono; il codice confrontato lega le due identità. Nome e IP rilevati prima della conferma non sono da soli prova dell’identità. L’autorizzazione resta salvata dopo il riavvio. **Rifiuta** nasconde il dispositivo, recuperabile con **Mostra dispositivi rifiutati**. Eliminando un dispositivo abbinato dalla mappa si revoca la pubblica aggiunta da Oaclis, senza toccare altre chiavi o chat.

Discovery: **UDP 23232** mediante broadcast IPv4 nella LAN; HTTP di annuncio: **TCP 23232**, solo `/peer/hello` accessibile dalla rete. UI e API operative restano riservate al loopback e alla sessione locale. In Linux nativo, consentire UDP/TCP 23232 e SSH nel firewall della sola rete prevista. Reti isolate, VLAN e Wi-Fi con isolamento client possono impedire il broadcast; resta l’aggiunta manuale per indirizzo. Questa versione non implementa discovery IPv6.

In WSL NAT, dopo un cambio IP riaprire il pannello e ripetere **Prepara collegamento** con gli IP rilevati al riavvio dell’app. Se Windows nega l’elevazione o il bridge non può essere avviato, usare le verifiche manuali seguenti. In questa versione la configurazione rete WSL usa NAT/portproxy: il percorso mirrored resta manuale.

### 4. Avviare e verificare SSH — terminale Ubuntu

```bash
command -v ssh
command -v sshd
sudo service ssh status
sudo service ssh start
ss -ltnp 'sport = :22'
whoami
hostname -I
```

Attendere un listener sulla porta 22. Se la distribuzione usa systemd, si può usare `sudo systemctl start ssh` al posto di `service`. Per avviare SSH anche alle successive accensioni di una distribuzione con systemd:

```bash
sudo systemctl enable ssh
```

Questo riguarda il server SSH, non un timer di sincronizzazione Oaclis. Senza systemd, riavviare SSH manualmente quando necessario. L’utente SSH deve poter leggere/scrivere i percorsi scelti; root è stato usato nella prova, ma il suo accesso non viene abilitato dall’installer. Non attivare il login root con password per questi passaggi.

### Avvio automatico Linux e WSL

Alla fine di `bash run.sh` (o `run.ps1 -Distro Ubuntu`), dopo la configurazione del ruolo, viene abilitato e avviato il servizio systemd `oaclis-<UID>.service`. Il servizio usa lo stesso utente e la stessa home dell'installazione: conserva pairing, autorizzazioni, chat, impostazioni e Sempre attivo. Riparte dopo un errore e al boot Linux o all'avvio della distribuzione WSL. La prima preparazione può richiedere sudo; le accensioni successive non richiedono una nuova conferma. Eseguire lo script come utente dell'installazione, senza anteporre sudo all'intero script.

Al termine della preparazione si apre la webapp `http://localhost:23232`, quando il servizio risponde. Su Windows è `run.ps1` ad aprire il browser su `http://127.0.0.1:23232`, dopo il completamento di `run.sh`, evitando che `localhost` risolva su un loopback IPv6 non raggiungibile; un Linux senza ambiente grafico mostra l'indirizzo senza installare un browser. Il servizio in background non apre finestre a ogni riavvio.

Su Linux avvio e apertura fanno parte di `bash run.sh`; su Windows si usa `.\run.ps1 -Distro Ubuntu`, che prepara Windows e richiama lo script Linux. Non serve un secondo comando.

Serve systemd attivo. In WSL aggiungere `[boot]` e `systemd=true` in `/etc/wsl.conf`, quindi riavviare la distribuzione. Questo non accende WSL al login Windows e non ripristina gli inoltri NAT Windows: per quelli resta `-PrepareNetwork` da PowerShell amministratore. Docker e macOS richiedono la gestione di avvio del rispettivo sistema ospitante e non ricevono un servizio systemd.

Controlli e allineamento riprendono normalmente; i trasferimenti restano subordinati al Play. Se Oaclis è già aperto fuori dal servizio, la preparazione non interrompe la sessione né eventuali trasferimenti: abilita il servizio per il prossimo avvio. Log e stato, sostituendo `<UID>` con `id -u`:

```bash
systemctl status oaclis-$(id -u).service
journalctl -u oaclis-$(id -u).service -n 50
```

### 5. Esporre SSH della WSL nella LAN — PowerShell amministratore

`run.ps1 -Distro Ubuntu` prepara anche inoltri e firewall usando gli IP attuali. Eseguirlo da PowerShell **come amministratore**. Dopo un riavvio o un cambio IP, dalla cartella della repository si può ripristinare soltanto la rete:

```powershell
.\run.ps1 -Distro Ubuntu -PrepareNetwork
```

Questa modalità non compila, installa o avvia Oaclis e non trasferisce dati. Prepara anche `127.0.0.1:23232 → WSL:23232` per aprire la webapp dal browser Windows. Se gli inoltri sono salvati ma non ascoltano, riavvia IP Helper e verifica nuovamente le porte. Configura SSH `2222 → 22`, Oaclis `23232 → 23232` e firewall TCP/UDP limitato alla sottorete locale; conserva gli inoltri già corretti. Salva gli IP rilevati in `wsl_network` nel file settings Windows. Distingue gli inoltri configurati dai servizi non ancora in ascolto. `-Check` controlla senza modificare la rete. Il ripristino avviene eseguendo lo script: non viene aggiunta un'attività automatica al riavvio.

I comandi seguenti sono la procedura manuale di diagnostica.

Questa procedura riguarda WSL2 in modalità NAT, usata nelle prove. In Linux nativo non serve portproxy. Per modalità mirrored e relativi firewall consultare la [documentazione Microsoft](https://learn.microsoft.com/en-us/windows/wsl/networking).

Rilevare separatamente l’IP Windows sulla rete fisica e quello della distribuzione:

```powershell
Get-NetIPConfiguration
Get-NetIPAddress -AddressFamily IPv4 | Select-Object InterfaceAlias,IPAddress
wsl -d Ubuntu hostname -I
netsh interface portproxy show all
```

Non usare l’IP dell’adattatore `vEthernet` come indirizzo LAN. Nei test KDell aveva Windows Wi-Fi `192.168.1.167` e WSL `172.29.128.195`: sono esempi, da sostituire con quelli attuali. Impostare queste variabili nella stessa finestra PowerShell:

```powershell
$OaclisLanIp = "192.168.1.167"
$OaclisWslIp = "172.29.128.195"
$OaclisSshPort = 2222
```

Prima verificare Windows → WSL:

```powershell
Test-NetConnection $OaclisWslIp -Port 22
```

Procedere solo se `TcpTestSucceeded` è `True`. Controllare il servizio usato da portproxy:

```powershell
Get-Service iphlpsvc
Set-Service iphlpsvc -StartupType Automatic
Start-Service iphlpsvc
```

Se non esiste un inoltro sulla coppia IP/porta scelta, aggiungerlo:

```powershell
netsh interface portproxy add v4tov4 listenaddress=$OaclisLanIp listenport=$OaclisSshPort connectaddress=$OaclisWslIp connectport=22
```

Se esiste già **il mapping Oaclis** ma è vecchio o non ascolta dopo il riavvio, rimuovere e ricreare solo quello; non cancellare tutti gli inoltri:

```powershell
netsh interface portproxy delete v4tov4 listenaddress=$OaclisLanIp listenport=$OaclisSshPort
netsh interface portproxy add v4tov4 listenaddress=$OaclisLanIp listenport=$OaclisSshPort connectaddress=$OaclisWslIp connectport=22
```

Se è cambiato anche l’IP LAN, il comando `delete` deve usare il vecchio IP mostrato da `show all`. Verificare la regola firewall esistente prima di crearne una nuova:

```powershell
Get-NetFirewallRule -Name "Oaclis-WSL-SSH" -ErrorAction SilentlyContinue
```

Se assente, creare la regola per la sola sottorete locale:

```powershell
New-NetFirewallRule -Name "Oaclis-WSL-SSH" -DisplayName "Oaclis WSL SSH" -Direction Inbound -Action Allow -Protocol TCP -LocalAddress $OaclisLanIp -LocalPort $OaclisSshPort -RemoteAddress LocalSubnet -Profile Any
```

Se esiste già ed è la regola Oaclis, aggiornare indirizzo e porta quando cambiano:

```powershell
Get-NetFirewallRule -Name "Oaclis-WSL-SSH" | Get-NetFirewallAddressFilter | Set-NetFirewallAddressFilter -LocalAddress $OaclisLanIp -RemoteAddress LocalSubnet
Get-NetFirewallRule -Name "Oaclis-WSL-SSH" | Get-NetFirewallPortFilter | Set-NetFirewallPortFilter -Protocol TCP -LocalPort $OaclisSshPort
Enable-NetFirewallRule -Name "Oaclis-WSL-SSH"
```

Verificare listener e accesso locale Windows:

```powershell
netsh interface portproxy show all
Get-NetTCPConnection -LocalPort $OaclisSshPort -State Listen
Test-NetConnection $OaclisLanIp -Port $OaclisSshPort
```

Dal **PowerShell dell’altro computer**, verificare anche l’accesso LAN, usando l’IP attuale del destinatario:

```powershell
Test-NetConnection 192.168.1.167 -Port 2222
```

Se Windows → WSL funziona ma la porta LAN no, verificare portproxy, `iphlpsvc` e firewall. Non disabilitare globalmente i firewall. Gli IP possono cambiare al riavvio: ripetere il rilevamento e aggiornare mapping/regola. La porta **2222** inoltra SSH verso **22**; **23232** è la porta Oaclis. Il percorso manuale qui sopra configura soltanto SSH: per la discovery usare **Prepara collegamento** nel pannello **+**, che prepara anche TCP/UDP 23232.

### 6. Verificare l’identità e autorizzare SSH — terminali Ubuntu

Sul **dispositivo destinazione**, leggere l’impronta della chiave host:

```bash
sudo ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub
```

Sul **dispositivo origine**, aprire una prima connessione (sostituire IP e utente):

```bash
ssh -o HostKeyAlgorithms=ssh-ed25519 -p 2222 kim@192.168.1.167
```

Confrontare l’impronta mostrata con quella letta direttamente sulla destinazione. Accettare solo se coincide. La chiave host identifica il dispositivo; è distinta dalla chiave di accesso dell’utente. Se viene segnalata una chiave cambiata, verificarne la causa prima di aggiornare `known_hosts`.

Sull’**origine**, creare una chiave dedicata solo se non è già presente:

```bash
mkdir -p ~/.ssh
chmod 700 ~/.ssh
if [ ! -e ~/.ssh/id_ed25519_oaclis ] && [ ! -e ~/.ssh/id_ed25519_oaclis.pub ]; then
    ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519_oaclis -C "oaclis"
fi
cat ~/.ssh/id_ed25519_oaclis.pub
```

Scegliere una passphrase; per l’automazione usare un agente SSH che la conservi durante la sessione. Nelle prove è stata usata una chiave dedicata senza passphrase. La privata resta sempre sull’origine.

Se il login SSH iniziale con password è consentito, autorizzare la pubblica dalla macchina origine:

```bash
ssh-copy-id -i ~/.ssh/id_ed25519_oaclis.pub -p 2222 kim@192.168.1.167
```

Se non è consentito, sul terminale **locale della destinazione**, come utente SSH prescelto, aggiungere la pubblica senza sovrascrivere le chiavi esistenti:

```bash
mkdir -p ~/.ssh
chmod 700 ~/.ssh
cat >> ~/.ssh/authorized_keys
```

Incollare la singola riga pubblica `ssh-ed25519 ...`, premere Invio e poi Ctrl+D. Quindi:

```bash
chmod 600 ~/.ssh/authorized_keys
```

Se la chiave ha una passphrase, sull’origine:

```bash
eval "$(ssh-agent -s)"
ssh-add ~/.ssh/id_ed25519_oaclis
```

Verificare accesso non interattivo e strumenti remoti:

```bash
ssh -o BatchMode=yes -o ConnectTimeout=5 -i ~/.ssh/id_ed25519_oaclis -p 2222 kim@192.168.1.167 'hostname; command -v rsync; command -v python3'
```

Per usare root sostituire `kim` con `root` e autorizzare la pubblica in `/root/.ssh/authorized_keys` dalla console root della destinazione. Non basta essere root sull’origine. Per operazioni in entrambe le direzioni, ripetere questi passaggi invertendo origine e destinazione: ciascuna macchina mantiene la propria chiave privata. Se anche l’altra macchina è WSL NAT, configurare anche il suo inoltro SSH.

### 7. Avviare Oaclis — terminale Ubuntu

In primo piano:

```bash
~/.local/bin/oaclis serve
```

Aprire **http://localhost:23232** nel browser Windows del dispositivo. Ctrl+C arresta il programma. In alternativa, per mantenerlo attivo chiudendo il terminale:

```bash
tmux new-session -d -s oaclis-client "$HOME/.local/bin/oaclis serve"
tmux list-sessions
curl -I http://localhost:23232/
```

Per vedere il processo, collegarsi alla sessione:

```bash
tmux attach -t oaclis-client
```

Ctrl+B, poi D, scollega il terminale lasciando Oaclis attivo; Ctrl+C arresta Oaclis. Non avviare due istanze sulla stessa porta. Dopo lo spegnimento di WSL/computer rilanciare il programma. Non vengono creati cron o servizi Oaclis.

Sul master, aggiungere il dispositivo dalla mappa: nelle impostazioni avanzate inserire l’IP Windows LAN, porta SSH `2222`, utente scelto e percorso della chiave privata **locale al master**. Per Linux nativo usare l’IP del dispositivo e la sua porta SSH, normalmente `22`.

La configurazione Oaclis è in `~/.config/oaclis`; le chat restano in `~/.codex` o nel `CODEX_HOME` impostato. Git distribuisce solo l’applicazione. Gli archivi Codex di root e dell’utente normale sono distinti.

## Aggiornamento

Arrestare Oaclis con Ctrl+C nel suo terminale/sessione tmux. Dalla cartella della repository:

```bash
cd ~/Oaclis
git pull --ff-only
bash scripts/install.sh client
~/.local/bin/oaclis serve
```

Il ruolo esistente viene conservato anche sul master. La reinstallazione non cancella configurazioni o chat.

## Stato della prima configurazione

Su KDell sono stati verificati download da GitHub, installazione con Rust/Cargo 1.93.1, ruolo client, avvio in tmux, risposta HTTP, CSS e catalogo delle chat. I controlli e il ripristino del portproxy dopo riavvio sono stati provati durante la configurazione delle due WSL; il firewall va verificato su ciascun Windows.

L’installer compila e installa; la rete Windows viene configurata dal pulsante **Prepara collegamento**, con elevazione locale. Il pannello **+** gestisce la discovery e la doppia autorizzazione. I passaggi manuali documentati restano disponibili per diagnosi e configurazioni particolari. Il ruolo client distingue il marchio; la UI locale è la stessa. Il percorso completo non è ancora stato ripetuto da zero su un nuovo Windows.

## Sviluppo e limiti

```bash
cargo test --locked
cargo run -- serve
```

Moduli Rust in `src/`, adattatori Python incorporati nel binario da `tools/`, interfaccia incorporata da `ui/`. Copie, confronto, checkpoint, regole e progresso rsync sono disponibili anche tramite CLI; avviare il binario senza argomenti per vedere i comandi.

Questa prima installazione supporta Linux e WSL. Windows nativo e macOS richiedono ancora adattamenti; Docker necessita di volumi persistenti e accesso SSH. Il passaggio monodirezionale definitivo con disattivazione automatica dopo la prima copia resta da completare.

Specifiche: [PROJECT.md](PROJECT.md). Funzioni UI: [docs/UI.md](docs/UI.md). Licenza: [Apache 2.0](LICENSE).


### Impostazioni condivise

I dispositivi autorizzati reciprocamente scambiano tramite SSH nomi, colori e icone di dispositivi/chat, collegamenti, categorie da sincronizzare e posizioni della mappa. Il dispositivo ha un’identità stabile: KDell mantiene lo stesso nome anche nella propria UI. Il master fornisce le impostazioni iniziali; successivamente le modifiche possono essere effettuate da qualsiasi dispositivo. Le modifiche offline sono persistenti e convergono alla riconnessione. Due modifiche concorrenti allo stesso valore vengono risolte in modo deterministico dalla revisione, con l’identità del dispositivo come spareggio. Le eliminazioni conservano una revisione per evitare ripristini da copie vecchie.

Lo stato condiviso è in `~/.config/oaclis/shared-settings.json`; le posizioni dei device sono in `positions.json` e gli spostamenti delle icone dei collegamenti in `link-positions.json`. Le posizioni ricevute usano la transizione UI condivisa, rispettando la preferenza di movimento ridotto. Le credenziali e gli alias SSH, l’autenticazione Codex e le analisi locali non vengono copiati. Ricevere un collegamento non avvia un secondo trasferimento di chat/file sul client. Il coordinamento continua fra dispositivi autorizzati anche se il master è spento. Le coordinate sono condivise; una finestra più piccola limita la visualizzazione ai propri bordi. Al primo caricamento della UI master, il layout precedentemente salvato nel browser viene importato nel file persistente. I temi della singola UI restano locali.

## Flussi indipendenti

Oaclis separa la replica delle impostazioni, i check automatici e i trasferimenti reali.
L'avvio normale usa `oaclis serve`, senza pause. Le seguenti variabili sono strumenti diagnostici da usare solo su richiesta esplicita; il match periodico resta separato dalla pausa del rilevamento locale:

| Variabile | Con valore `1` |
| --- | --- |
| `OACLIS_PAUSE_SETTINGS` | Ferma la replica di nomi, icone, colori, posizioni, collegamenti e inventari salvati. |
| `OACLIS_PAUSE_CHECKS` | Ferma i check automatici. Il refresh manuale del singolo blocco resta disponibile. |
| `OACLIS_PAUSE_TRANSFERS` | Ferma i trasferimenti reali di chat, percorsi e app. |

Per allineare solo le impostazioni, lasciando fermi check automatici e trasferimenti:

```sh
OACLIS_PAUSE_SETTINGS=0 OACLIS_PAUSE_CHECKS=1 OACLIS_PAUSE_TRANSFERS=1 oaclis serve
```

Le impostazioni viaggiano tramite il collegamento SSH già autorizzato, senza avviare
trasferimenti dei contenuti. Per compatibilità, `OACLIS_PAUSED=1` resta il valore
predefinito di pausa per check e trasferimenti quando le rispettive variabili non
sono specificate; non sospende la replica delle impostazioni.

## Ciclo dei collegamenti

I tre passaggi sono separati:

1. Ogni dispositivo rileva localmente Percorsi e App e salva l'inventario nel DB. Le analisi AI completate restano salvate; il refresh manuale riguarda soltanto il modulo scelto. Aprire un pannello non avvia check.
2. Ogni 60 secondi il match confronta le chat e i risultati locali dei moduli attivi, senza eseguire trasferimenti. I contenuti delle cartelle hanno una prova SHA-256; le impronte dei file immutati vengono riutilizzate localmente e non inviate alla UI o agli altri dispositivi.
3. Play compare soltanto in presenza di differenze e inventari pronti. Il trasferimento parte esclusivamente con Play, anche la prima volta, in ordine Chat → Percorsi → App.

La chat più recente decide il verso per tutti i moduli attivi. Un'estensione della cronologia prevale sulla copia precedente; in caso di storie divergenti si usa l'ultima attività UTC registrata nella chat, non la data di copia del file. Se non è possibile distinguere le copie, il collegamento segnala conflitto. Con chat identiche rimane il verso dell'ultimo confronto, oppure la sorgente scelta con il drag.

- Monodirezionale: un solo trasferimento riuscito, poi stato `transferred`.
- Bidirezionale: confronto periodico e trasferimenti manuali con Play.
- Offline: attesa, senza tentativi di trasferimento; il Play già richiesto resta in attesa della riconnessione.
- I toggle Chat, Percorsi e App sono indipendenti e valgono per tutti i collegamenti della chat.
- Il loading ad arco indica un check in corso. Durante il trasferimento gli anelli mostrano l'avanzamento dei soli moduli con variazioni; alla fine tornano Modifica e Console.

`chat_match_interval_seconds` in `~/.config/oaclis/settings.json` regola il
controllo periodico: valore predefinito 60 secondi, intervallo 10–3600 secondi.
L'inventario di Percorsi e App resta persistente: non si rilancia l'analisi AI a
ogni nuovo messaggio. I rispettivi refresh manuali aggiornano le definizioni.

L'esecuzione usa i moduli abilitati, in ordine Chat → Percorsi → App. Il modulo App
trasferisce l'inventario della sorgente, le configurazioni e i plugin/skill previsti
dal motore esistente; questa versione non installa automaticamente i pacchetti di
sistema sulla destinazione.

Lo stato del ciclo risiede in `chat_links.value.flow`, viene replicato con il
collegamento e contiene l'esecutore, una generazione, il primo completamento,
le prove dell'ultimo confronto e l'eventuale richiesta di Play. La generazione
impedisce che un lavoro vecchio ricrei un collegamento cancellato e ricreato.
Il confronto risiede in `src/link_flow.rs`; `src/automation.rs` lo pianifica;
`src/chat.rs` confronta gli archivi in Rust; il motore SSH/rsync esegue i trasferimenti.

## Moduli del codice

- `src/web.rs`: route HTTP, autenticazione locale e asset dell'interfaccia.
- `src/catalog.rs`: lettura dei cataloghi locali e remoti.
- `src/initial_inventory.rs`: rilevamento iniziale e refresh manuale.
- `src/transfer_job.rs`: esecuzione dei trasferimenti autorizzati.
- `ui/app.js`: stato condiviso, API, impostazioni generali, avvio e polling.
- `ui/map.js`: dispositivi, collegamenti, badge e lista delle chat.
- `ui/panels.js`: pannelli contestuali, personalizzazione e dettagli dei moduli.
- `ui/console.js`: scelta del dispositivo e console delle chat.

I moduli frontend usano gli script differiti già previsti dall'applicazione, senza
bundler o nuove dipendenze. Le funzioni condividono lo stato del controller:
`panels.js`, `map.js` e `console.js` definiscono i componenti prima che `app.js`
registri gli eventi e avvii il polling. Il caricamento è dichiarato in `index.html`.

### Comandi sulla card del dispositivo

Accanto alla matita, Riavvia riavvia soltanto Oaclis e chiede conferma. Attendere la conclusione dei trasferimenti e delle analisi prima di riavviare. I comandi remoti passano dall’accesso SSH già configurato.

Sempre attivo impedisce lo standby mentre Oaclis è aperto e conserva la scelta al riavvio: su Linux usa `systemd-inhibit`, su macOS `caffeinate`, in WSL una richiesta di alimentazione Windows. Non installa servizi. Nei container non può bloccare lo standby dell’host; se il sistema non concede il blocco mostra l’errore e non lo dichiara attivo. La sospensione esplicita dell’utente resta dipendente dalle regole del sistema operativo.

Sulle card degli slave l’icona V è allineata agli altri comandi: fissa se allineata al master, lampeggiante se serve un aggiornamento. Il tooltip contiene solo la versione installata. Un clic verifica la versione effettiva e, se diversa, avvia direttamente l’aggiornamento senza conferma. Il confronto usa l’identità dei sorgenti oltre alla versione media visualizzata. L’aggiornamento avviene solo su richiesta e conserva configurazione e chat; una repo modificata o divergente blocca il comando.

Disponibilità dell’app, versione installata e stato reale di “Sempre attivo” viaggiano insieme allo scambio SSH delle impostazioni già esistente. La UI legge questa fotografia, senza verifiche SSH al passaggio del mouse e senza dipendere dagli annunci LAN per la versione. Una fotografia più vecchia di 30 secondi non viene usata come stato corrente. “Sempre attivo” è telemetria del device, non un’impostazione da applicare agli altri.

Su Linux, l’utente che esegue Oaclis deve essere autorizzato a inibire lo standby. Appartenere al gruppo sudo non concede automaticamente questo permesso: se manca, il comando segnala l’utente interessato e il blocco resta disattivato. La gestione guidata di questo requisito è annotata in docs/TASKS.md.


### Preparare Sempre attivo

La preparazione non attiva il blocco: dopo il setup si usa il pulsante Sempre attivo della card Oaclis. Non occorre reinstallare l'app.

Su Linux, dal proprio utente nella repository:

```sh
bash run.sh --prepare-awake
```

La prima volta sudo puo chiedere la password: viene installata soltanto una regola polkit per l'utente corrente e le due azioni logind di inibizione sleep/idle. Oaclis continua a girare senza root. Non vengono modificati i timeout di sospensione, creati servizi o timer.

Su Windows con WSL, dalla repository in PowerShell:

```powershell
.\run.ps1 -Distro Ubuntu -PrepareAwake
```

Questa preparazione è già inclusa nel normale `run.ps1`; l’opzione serve soltanto a riparare il supporto separatamente. Sostituire Ubuntu con il nome della distribuzione Oaclis. È PowerShell a copiare e verificare il supporto; `run.sh` dentro WSL gestisce solo la parte Linux e non prepara Windows né apre il suo browser. Il setup crea `%LOCALAPPDATA%\Oaclis\keep-awake.ps1`; la normale richiesta di Oaclis usa questo helper Windows e lo rilascia quando il blocco viene spento o l'app termina. La policy di esecuzione PowerShell viene impostata soltanto sul processo helper, senza cambiare quella del sistema.


### Stato operativo in Rust e JSON

Oaclis carica in memoria quattro file in `~/.config/oaclis/`, corrispondenti alle precedenti tabelle: `appearance.json`, `analysis.json`, `chat_links.json` e `transfers.json`. `appearance.json` conserva per ogni chiave `value`, `hidden` e `sync`. Gli altri file mantengono le chiavi e gli oggetti gia' usati nelle rispettive tabelle. Le letture non avviano processi e i salvataggi identici non riscrivono i file.

L'importazione iniziale legge il precedente `appearance.sqlite` senza modificarlo; se i quattro JSON sono gia' presenti, SQLite non viene piu' usato come archivio Oaclis. I database appartenenti a Codex sono consultati soltanto in lettura per ricavare i titoli. Non si convertono chat, cartelle o dati dell'applicazione Codex.

Catalogo, confronto delle chat e scambio delle impostazioni fra device sono nativi Rust. SSH conserva alias, identita' e autorizzazioni esistenti; il comando interno `state-request` inoltra i messaggi all'app gia' avviata tramite un socket locale accessibile allo stesso utente. Non viene avviato un worker Python. Gli script Python di analisi, installazione e gestione del sistema restano indipendenti da questo flusso.

Ogni file viene sostituito atomicamente. Un salvataggio che coinvolge piu' tabelle usa un registro temporaneo `.state-commit.json`, recuperato all'avvio e rimosso a completamento, per evitare modifiche parziali dopo un'interruzione. `shared-settings.json`, i file dei device e le posizioni conservano il loro ruolo e formato esistenti.
