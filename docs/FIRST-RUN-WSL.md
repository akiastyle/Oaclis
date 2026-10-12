# Primo avvio e accesso SSH in WSL

Registro dei passaggi osservati durante il collegamento del secondo portatile. Questo documento alimenterà il wizard: non è ancora un'implementazione né una procedura interamente verificata.

## Dati osservati nella prova

| Ambiente | Indirizzo osservato | Esito |
|---|---|---|
| WSL locale | `172.20.94.232` | Client SSH disponibile; nessun server in ascolto sulla porta 22 |
| WSL secondo portatile | `172.29.128.195` | Collegamento TCP da qui alla porta 22 scaduto |
| Windows secondo portatile, Wi-Fi | `192.168.1.167` | Collegamento TCP da qui alla porta 22 scaduto |
| Windows secondo portatile, adattatore WSL | `172.29.128.1` | Indirizzo della rete virtuale, distinto dall'indirizzo LAN Wi-Fi |

Gli indirizzi sono dati della prova, non valori da inserire nel codice. Possono cambiare; il wizard deve rilevarli. Non dedurre la causa di un timeout: server assente, instradamento e firewall vanno verificati separatamente.

## Controlli iniziali

Su ciascun ambiente Ubuntu coinvolto:

```bash
command -v ssh
command -v sshd
ip -brief address
service ssh status
ss -ltnp 'sport = :22'
```

In questa macchina è stato usato anche `systemctl is-active ssh`. Il wizard dovrà distinguere sistemi con systemd e modalità di avvio disponibili, senza considerare l'assenza di un servizio prova sufficiente dell'assenza del pacchetto.

Sul Windows del secondo portatile:

```powershell
Get-NetIPConfiguration
```

L'utente ha identificato l'interfaccia Wi-Fi attiva con gateway LAN. Il wizard dovrà distinguere interfacce fisiche, virtuali e disconnesse; con più reti valide dovrà permettere una scelta visuale.

## Verifica di raggiungibilità

È stata provata una connessione TCP con timeout di 5 secondi alla porta 22, prima dell'autenticazione. Se disponibile, si legge il banner SSH con un timeout aggiuntivo di 3 secondi. Nessuna chiave è stata accettata e nessun login è stato eseguito.

Esito su entrambi gli indirizzi del secondo portatile: timeout. Il successivo controllo locale nella WSL ha restituito:

```text
Unit ssh.service could not be found.
```

`ss` non ha mostrato listener sulla porta 22. Questo è il punto raggiunto prima di proporre l'installazione.

## Installazione e avvio proposti

Nella WSL del secondo portatile, come root:

```bash
apt update
apt install -y openssh-server
service ssh start
ss -ltnp 'sport = :22'
```

L'utente ha confermato l'installazione e mostrato listener `sshd` su `0.0.0.0:22` e `[::]:22`, associati anche a systemd. Il server è quindi in ascolto nella WSL; l'accessibilità dalla LAN è un controllo distinto. Se l'app è avviata come utente ordinario, l'installazione e l'avvio devono usare un meccanismo locale di elevazione. Il wizard deve mostrare l'azione, verificarne l'esito e mantenere gli errori visibili.

L'installazione con apt può richiedere accesso ai repository dei pacchetti: ciò è distinto dall'esecuzione e dalla sincronizzazione Oaclis, che devono restare nella LAN. L'app non deve installare nuovamente un server già presente né sostituirne la configurazione.

## Passaggi successivi ancora da verificare

Dopo l'avvio del server, le connessioni da qui a `172.29.128.195:22` e `192.168.1.167:22` sono ancora scadute. Il listener Ubuntu è verificato; l'esposizione tramite Windows resta da configurare e verificare.

Proposta per il secondo portatile, in PowerShell amministratore: verificare prima il collegamento Windows → WSL, poi inoltrare `192.168.1.167:2222` verso `172.29.128.195:22` e consentire TCP 2222 soltanto dalle sottoreti locali. La porta 2222 mantiene distinta questa prova dall'eventuale SSH Windows sulla 22.

```powershell
Test-NetConnection 172.29.128.195 -Port 22
netsh interface portproxy show all
```

Prima di applicare l'inoltro controllare `TcpTestSucceeded: True` e l'assenza di un inoltro esistente su `192.168.1.167:2222`. I comandi di modifica previsti sono:

```powershell
netsh interface portproxy add v4tov4 listenaddress=192.168.1.167 listenport=2222 connectaddress=172.29.128.195 connectport=22
New-NetFirewallRule -Name "Oaclis-WSL-SSH" -DisplayName "Oaclis WSL SSH" -Direction Inbound -Action Allow -Protocol TCP -LocalAddress 192.168.1.167 -LocalPort 2222 -RemoteAddress LocalSubnet -Profile Any
```

L'utente ha confermato `TcpTestSucceeded: True` per Windows → `172.29.128.195:22`, con sorgente `172.29.128.1` sull'adattatore WSL. `netsh interface portproxy show all` ha restituito un elenco vuoto. L'inoltro sopra può quindi essere proposto senza sostituire un mapping esistente.

L'utente ha mostrato l'inoltro attivo `192.168.1.167:2222` → `172.29.128.195:22`. La connessione da questa WSL alla porta 2222 ha ricevuto il banner `SSH-2.0-OpenSSH_10.2p1 Ubuntu-2ubuntu3.7`: raggiungibilità LAN → server SSH WSL verificata. L'output specifico della creazione della regola firewall non è stato fornito; non è possibile attribuire l'accesso a una particolare regola.

La chiave host ED25519 rilevata con ssh-keyscan ha impronta `SHA256:olDb6lA7RQa1Fm5GL8MlfwvQ2Z/ZZyv/nQTo4tWBzqg`. L'utente ha mostrato la stessa impronta dalla WSL remota: identità verificata. ssh-keyscan non autentica da solo il dispositivo.

La macchina remota contiene due archivi distinti: `/home/kim/.codex`, appartenente a kim, e `/root/.codex`, appartenente a root. Non presumere che l'archivio dell'utente SSH sia l'unico da trasferire.

Un tentativo BatchMode con la chiave locale preesistente verso `kim@192.168.1.167:2222`, usando solo la chiave host verificata, è stato rifiutato (`Permission denied`). È stata preparata una chiave ED25519 dedicata a Oaclis in `/root/.ssh/id_ed25519_oaclis`, senza passphrase per l'automazione, con protezioni standard OpenSSH. La privata rimane locale.

L'utente ha poi scelto root per consentire installazione e configurazione e ha autorizzato la pubblica in `/root/.ssh/authorized_keys` sul portatile. Il login come root è riuscito. L'alias locale `oaclis-wsl-2` conserva il collegamento con controllo rigoroso della chiave host. Prove di trasferimento e ripresa Codex e preparazione dell'accesso inverso sono documentate in [SSH-TESTS.md](SSH-TESTS.md).

Prima di creare una regola firewall verificare che il nome non appartenga già a una regola esistente. Il wizard dovrà rendere configurabili gli indirizzi e controllare il risultato di ogni comando, evitando sovrascritture.

Fonte per l'inoltro WSL: [documentazione Microsoft](https://learn.microsoft.com/en-us/windows/wsl/networking#accessing-a-wsl-2-distribution-from-your-local-area-network-lan).

1. Server in ascolto nella WSL remota: verificato tramite output fornito dall'utente.
2. Verificare come Windows espone la WSL alla LAN nella configurazione effettiva, prima di scegliere un intervento.
3. Se necessario, predisporre una porta Windows dedicata, inoltro verso la WSL e regola firewall limitata alla rete prevista. Non sono ancora stati definiti o eseguiti comandi per questi interventi.
4. Verificare nuovamente raggiungibilità e banner dalla macchina origine.
5. Verificare la chiave host e configurare l'abbinamento con conferma sulle due macchine.
6. Autorizzare le chiavi pubbliche per l'utente selezionato, conservando chiavi private e accessi esistenti. Non presumere che il login root sia abilitato solo perché il terminale locale è root.
7. SSH nelle due direzioni verificato: server locale installato e avviato, chiavi pubbliche autorizzate, inoltro Windows locale 2222 configurato dall'utente e login remoto → ROG riuscito. Dettagli in SSH-TESTS.md.
8. Testare trasferimento di dati dimostrativi, verifica di integrità e disconnessione.

## Comportamento richiesto al wizard

- Prima controllare, poi proporre solo le azioni mancanti.
- Rilevare indirizzi, utente, porta e disponibilità degli strumenti; ridurre la digitazione attraverso scelte e conferme.
- Eseguire i comandi localmente sul dispositivo interessato: non richiedere SSH funzionante per configurare il primo accesso SSH.
- Separare stato del server, accessibilità dalla LAN, verifica dell'identità e autenticazione.
- Distinguere privilegi Ubuntu e privilegi Windows; non dare per scontato che l'app abbia entrambi.
- Non modificare automaticamente firewall o configurazioni SSH senza una scelta esplicita nel wizard.
- Non registrare password, chiavi private o credenziali nei log.
- Conservare ciò che Oaclis configura per poterlo aggiornare o rimuovere senza toccare regole e accessi estranei.
- Gestire un eventuale cambio dell'IP interno WSL senza mantenere un inoltro non valido.

Proseguire questo registro con comandi effettivamente eseguiti, risultati e controlli successivi. Tradurre i passaggi in codice soltanto dopo averne verificato il funzionamento sui due dispositivi.
