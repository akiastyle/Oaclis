# Sourced by run.sh after the required prerequisites are ready.
prepare_codex() {
    if command -v codex >/dev/null; then
        if codex --version; then return 0; fi
        echo 'Codex trovato ma non funzionante: nessuna reinstallazione automatica.'
        return 1
    fi
    if [[ ! -t 0 ]]; then echo 'Codex non rilevato; installazione opzionale saltata (terminale non interattivo).'; return 0; fi
    if ! confirm 'Codex non rilevato. Vuoi installarlo?'; then echo 'Codex non installato; la preparazione di Oaclis continua.'; return 0; fi
    local node_major=0
    if command -v node >/dev/null; then node_major=$(node -p 'process.versions.node.split(".")[0]') || return 1; fi
    if [[ ! "$node_major" =~ ^[0-9]+$ ]]; then echo 'Versione Node.js non riconosciuta.'; return 1; fi
    if (( node_major < 16 )) || ! command -v npm >/dev/null; then
        echo 'Serve Node.js con npm. Verranno preparati Node.js 22 e npm tramite NVM nella home dell’utente.'
        if ! confirm 'Procedere con Node.js/npm e il caricamento di NVM nella configurazione della shell?'; then echo 'Installazione Codex saltata.'; return 0; fi
        export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
        if [[ ! -s "$NVM_DIR/nvm.sh" ]]; then
            if [[ -e "$NVM_DIR" ]]; then echo 'Cartella NVM già presente ma incompleta: nessuna sovrascrittura.'; return 1; fi
            git clone --depth 1 --branch v0.40.3 https://github.com/nvm-sh/nvm.git "$NVM_DIR" || return 1
        fi
        set +u
        if ! source "$NVM_DIR/nvm.sh"; then set -u; return 1; fi
        if ! nvm install 22 || ! nvm alias default 22; then set -u; return 1; fi
        set -u
        local profile="$HOME/.bashrc"
        [[ "${SHELL:-}" != *zsh ]] || profile="$HOME/.zshrc"
        if [[ -L "$profile" ]]; then echo 'Configurazione shell tramite link: caricare NVM manualmente.'
        elif ! grep -Fq 'nvm.sh' "$profile" 2>/dev/null; then
            printf '\n# NVM: Node.js/npm per Codex\nexport NVM_DIR=%q\n[ ! -s "$NVM_DIR/nvm.sh" ] || . "$NVM_DIR/nvm.sh"\n' "$NVM_DIR" >> "$profile" || return 1
        fi
    fi
    node --version || return 1
    npm --version || return 1
    echo 'Installazione: npm install --global @openai/codex@0.162.1'
    if ! confirm 'Confermare l’installazione di Codex 0.162.1?'; then echo 'Codex non installato.'; return 0; fi
    # A user prefix avoids sudo npm and does not change the existing npm configuration.
    if [[ "$(command -v node)" == "${NVM_DIR:-$HOME/.nvm}/versions/node/"* ]]; then
        npm install --global @openai/codex@0.162.1 || return 1
    else
        echo 'Codex verrà installato in ~/.local; aggiungere ~/.local/bin al PATH della propria shell se assente.'
        npm install --global --prefix "$HOME/.local" @openai/codex@0.162.1 || return 1
        export PATH="$HOME/.local/bin:$PATH"
    fi
    hash -r
    codex --version || return 1
    echo 'Codex verificato. L’autenticazione è un passaggio separato; aprire una nuova shell se è stato configurato NVM.'
}
