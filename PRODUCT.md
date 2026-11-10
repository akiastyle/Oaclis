# Product
<!-- impeccable:product-schema 1 -->
## Platform
web
## Stack
Nucleo Rust esistente. Prima UI HTML/CSS/JavaScript senza framework, servita localmente da Rust: ipotesi proposta, modificabile.
## Users
Proprietario di più dispositivi Ubuntu/WSL con Codex, che sposta il lavoro da un dispositivo all'altro.
## Product Purpose
Copiare o aggiornare chat e cartelle di progetto fra dispositivi nella stessa LAN con regole esplicite.
## Operating Context
WSL master configura le regole; SSH e rsync eseguono i trasferimenti. Porta di controllo scelta: 23232. Il master non deve restare sempre acceso.
## Capabilities and Constraints
Niente cloud, Git come trasporto, cron o timer systemd. Originali conservati. Copia e sincronizzazione sono operazioni distinte. Dispositivi e regole modificabili. Drag&drop prepara una proposta; spostare nodi modifica solo il layout. Avanzamento reale, conflitti dichiarati.
## Brand Commitments
Oaclis. UI minimale, moderna e ordinata, temi chiaro/scuro esclusivamente in bianco, nero e grigi. Solo chat Codex nella libreria; nessuna creazione di chat o progetti in Oaclis. Percorsi derivati dalla chat. Riferimenti espliciti Tenzen Studio, ElevenLabs; logica della mappa SPAAIC locale.
## Evidence on Hand
Registro dispositivi e regole in ~/.config/oaclis. QR3D e portatile WSL realmente configurati. Catalogo Codex locale. Mac e Jetson non ancora registrati.
## Product Principles
Poche azioni esplicite; riutilizzo del nucleo Rust; strumenti Linux affidabili; dati reali senza stati inventati; moduli separati.

Correzione UI: la libreria mostra esclusivamente chat esistenti di Codex. Creazione di chat e progetti in Codex, non in Oaclis. I percorsi sono derivati/associati alla chat, non voci autonome. Le regole Rust esistenti restano disponibili da CLI; la derivazione dei percorsi di progetto non è ancora collegata alla UI.
