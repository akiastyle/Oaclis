# Oaclis

Applicazione Rust per gestire copie e sincronizzazioni di chat Codex, cartelle e ambiente tramite SSH e rsync. Git distribuisce il programma; non trasporta le chat.

## Installazione client Linux / WSL

Servono Rust con Cargo (compatibile con edition 2024), Python 3, OpenSSH e rsync. Per aprire le chat servono anche Codex e tmux. Codex deve essere configurato sul dispositivo con il proprio accesso.

Su Ubuntu, installare le dipendenze mancanti:

```bash
sudo apt update
sudo apt install -y git build-essential python3 openssh-client openssh-server rsync tmux
```

Installare Rust tramite [rustup](https://rustup.rs/) se Cargo non è presente. Poi:

```bash
git clone https://github.com/akiastyle/Oaclis.git
cd Oaclis
bash scripts/install.sh client
~/.local/bin/oaclis serve
```

Aprire **http://localhost:23232** nel browser del dispositivo. Il comando resta in esecuzione; Ctrl+C arresta Oaclis. Non vengono aggiunti cron o servizi systemd.

Per il master usare `bash scripts/install.sh master` alla prima installazione. L’installer conserva un ruolo già impostato e tutte le configurazioni esistenti. La configurazione è in `~/.config/oaclis`, le chat restano in `~/.codex`. Il server UI ascolta solo su loopback.

## Aggiornamento

Arrestare Oaclis, quindi dalla cartella del repository:

```bash
git pull --ff-only
bash scripts/install.sh client
~/.local/bin/oaclis serve
```

## Collegamento

Il dispositivo deve essere raggiungibile via SSH con chiave autorizzata e identità host verificata. Aggiungerlo dalla mappa del master tramite indirizzo, porta e utente nelle impostazioni avanzate. In WSL con rete NAT può servire l’inoltro della porta SSH su Windows: [procedura e verifiche](docs/FIRST-RUN-WSL.md).

La scoperta automatica LAN e l’accettazione reciproca tra due app non sono ancora implementate: installare il client non configura automaticamente SSH. Il ruolo client attualmente distingue il marchio; la UI locale è la stessa. Non vengono trasferite chiavi private o credenziali Codex.

## Sviluppo e limiti

```bash
cargo test --locked
cargo run -- serve
```

Moduli Rust in `src/`, adattatori Python incorporati nel binario da `tools/`, interfaccia incorporata da `ui/`. Copie, confronto, checkpoint, regole e progresso rsync sono disponibili anche tramite CLI; avviare il binario senza argomenti per vedere i comandi.

Questa prima installazione supporta Linux e WSL. Windows nativo e macOS richiedono ancora adattamenti; Docker necessita di volumi persistenti e accesso SSH. Il passaggio monodirezionale definitivo con disattivazione automatica dopo la prima copia resta da completare.

Specifiche: [PROJECT.md](PROJECT.md). Funzioni UI: [docs/UI.md](docs/UI.md). Licenza: [Apache 2.0](LICENSE).
