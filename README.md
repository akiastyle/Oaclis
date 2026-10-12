# Oaclis

Applicazione Rust per gestire copie e sincronizzazioni di chat Codex, cartelle e ambiente tramite SSH e rsync. Git distribuisce il programma; non trasporta le chat.

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

## Installazione e prima configurazione: Ubuntu / WSL

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

Codex deve essere già installato e autenticato sul dispositivo; non copiare le credenziali da un altro computer. Per le console delle chat verificare:

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

Aprire `http://localhost:23232` su ciascuno e premere **+** nella toolbar: il pannello cerca gli altri Oaclis. Su WSL premere **Configura rete WSL** e accettare la richiesta amministrativa Windows su ciascun computer. Vengono configurati soltanto gli inoltri SSH `2222 → 22`, controllo `23232 → 23232` e le regole firewall Oaclis TCP/UDP limitate alla sottorete locale. Il ponte di discovery Windows parte con Oaclis e termina quando l’app non risponde più; non si aggiungono servizi o attività pianificate.

Quando compare l’altro dispositivo, confrontare il codice mostrato nelle due UI e premere **Il codice coincide · Accetta su entrambe**. Una sola accettazione lascia il collegamento in attesa. Dopo la seconda, Oaclis autorizza le chiavi pubbliche dedicate, salva l’identità host SSH e aggiunge il dispositivo alla mappa; il controllo SSH della mappa verifica poi l’accesso operativo. Le chiavi private restano sul dispositivo che le ha generate. Non vengono copiati chat, file o credenziali durante l’abbinamento.

Gli annunci LAN sono firmati con Ed25519 e scadono; il codice confrontato lega le due identità. Nome e IP rilevati prima della conferma non sono da soli prova dell’identità. L’autorizzazione resta salvata dopo il riavvio. **Rifiuta** nasconde il dispositivo, recuperabile con **Mostra dispositivi rifiutati**. Eliminando un dispositivo abbinato dalla mappa si revoca la pubblica aggiunta da Oaclis, senza toccare altre chiavi o chat.

Discovery: **UDP 23232** mediante broadcast IPv4 nella LAN; HTTP di annuncio: **TCP 23232**, solo `/peer/hello` accessibile dalla rete. UI e API operative restano riservate al loopback e alla sessione locale. In Linux nativo, consentire UDP/TCP 23232 e SSH nel firewall della sola rete prevista. Reti isolate, VLAN e Wi-Fi con isolamento client possono impedire il broadcast; resta l’aggiunta manuale per indirizzo. Questa versione non implementa discovery IPv6.

In WSL NAT, dopo un cambio IP riaprire il pannello e ripetere **Configura rete WSL** con gli IP rilevati al riavvio dell’app. Se Windows nega l’elevazione o il bridge non può essere avviato, usare le verifiche manuali seguenti. In questa versione la configurazione rete WSL usa NAT/portproxy: il percorso mirrored resta manuale.

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

### 5. Esporre SSH della WSL nella LAN — PowerShell amministratore

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

Se Windows → WSL funziona ma la porta LAN no, verificare portproxy, `iphlpsvc` e firewall. Non disabilitare globalmente i firewall. Gli IP possono cambiare al riavvio: ripetere il rilevamento e aggiornare mapping/regola. La porta **2222** inoltra SSH verso **22**; **23232** è la porta Oaclis. Il percorso manuale qui sopra configura soltanto SSH: per la discovery usare **Configura rete WSL** nel pannello **+**, che prepara anche TCP/UDP 23232.

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

L’installer compila e installa; la rete Windows viene configurata dal pulsante **Configura rete WSL**, con elevazione locale. Il pannello **+** gestisce la discovery e la doppia autorizzazione. I passaggi manuali documentati restano disponibili per diagnosi e configurazioni particolari. Il ruolo client distingue il marchio; la UI locale è la stessa. Il percorso completo non è ancora stato ripetuto da zero su un nuovo Windows.

## Sviluppo e limiti

```bash
cargo test --locked
cargo run -- serve
```

Moduli Rust in `src/`, adattatori Python incorporati nel binario da `tools/`, interfaccia incorporata da `ui/`. Copie, confronto, checkpoint, regole e progresso rsync sono disponibili anche tramite CLI; avviare il binario senza argomenti per vedere i comandi.

Questa prima installazione supporta Linux e WSL. Windows nativo e macOS richiedono ancora adattamenti; Docker necessita di volumi persistenti e accesso SSH. Il passaggio monodirezionale definitivo con disattivazione automatica dopo la prima copia resta da completare.

Specifiche: [PROJECT.md](PROJECT.md). Funzioni UI: [docs/UI.md](docs/UI.md). Licenza: [Apache 2.0](LICENSE).
