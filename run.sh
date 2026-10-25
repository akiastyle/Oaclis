#!/usr/bin/env bash
# run.sh v1.5.234
# Prepare prerequisites, compile, configure and enable application startup.
set -euo pipefail
if [[ ${1:-} == --desktop-test ]]; then
    exec bash "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/scripts/desktop-test.sh" --prepare
fi
case "${1:-}" in ''|--check|--prepare-awake) ;; *) echo 'Uso: bash run.sh [--check|--prepare-awake]' >&2; exit 2;; esac
check_only=${1:-}
if [[ "$check_only" == --prepare-awake ]]; then
    awake_binary="$HOME/.local/bin/oaclis"
    [[ -x "$awake_binary" ]] || { echo 'Installare prima Oaclis con bash run.sh.' >&2; exit 1; }
    exec "$awake_binary" prepare-awake "${@:2}"
fi
system=$(uname -s)
case "$system" in Linux|Darwin) ;; *) echo 'Usare run.ps1 su Windows nativo.' >&2; exit 1;; esac
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
# Reuse an existing Node installation before declaring Codex absent.
if ! command -v codex >/dev/null && [[ -s "${NVM_DIR:-$HOME/.nvm}/nvm.sh" ]]; then
    export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
    set +u
    if ! source "$NVM_DIR/nvm.sh"; then echo 'NVM esistente non caricato; continuo con i comandi nel PATH.'; fi
    set -u
fi
container=no; wsl=no
if [[ -f /.dockerenv || -f /run/.containerenv ]] || { [[ -r /proc/1/cgroup ]] && grep -Eq 'docker|containerd|kubepods|podman' /proc/1/cgroup; }; then container=yes; fi
if [[ "$system" == Linux ]] && { [[ -n ${WSL_DISTRO_NAME:-} ]] || [[ $(uname -r) == *[Mm]icrosoft* ]]; }; then wsl=yes; fi
printf 'Sistema: %s\nContainer: %s\nKernel WSL: %s\n' "$system" "$container" "$wsl"
if [[ "$wsl" == yes && "$container" == no ]]; then
    echo 'Windows: rete, standby e browser sono gestiti da run.ps1.'
elif [[ "$container" == yes ]]; then
    echo '[Nota] In container il blocco standby deve essere gestito sul sistema host.'
fi
missing=(); packages=()
check() {
    if command -v "$1" >/dev/null; then printf '[OK] %s\n' "$1";
    else printf '[Manca] %s\n' "$1"; missing+=("$1"); [[ -z "$2" ]] || packages+=("$2"); fi
}
if [[ "$system" == Linux ]]; then
    for item in git:git curl:curl ssh:openssh-client ssh-keygen:openssh-client rsync:rsync tmux:tmux cc:build-essential ip:iproute2; do check "${item%%:*}" "${item#*:}"; done
else
    for item in git:git curl:curl ssh:openssh ssh-keygen:openssh rsync:rsync tmux:tmux cc:; do check "${item%%:*}" "${item#*:}"; done
    if ! xcode-select -p >/dev/null 2>&1; then
        echo '[Manca] Strumenti Apple: xcode-select --install'
        missing+=(apple-tools)
    fi
fi
check cargo ''; check rustc ''
if command -v rustc >/dev/null; then
    version=$(rustc --version); printf '%s\n' "$version"
    read -r major minor _ <<< "$(printf '%s' "$version" | sed -E 's/^[^ ]+ ([0-9]+)\.([0-9]+).*/\1 \2/')"
    if [[ ! "$major" =~ ^[0-9]+$ || ! "$minor" =~ ^[0-9]+$ ]]; then echo 'Versione Rust non riconosciuta.' >&2; exit 1; fi
    if (( major < 1 || (major == 1 && minor < 85) )); then echo '[Da aggiornare] Rust: edition 2024 richiede almeno 1.85; usare stable per Cargo.lock.'; missing+=(rustc); fi
fi
for tool in node npm codex; do
    if command -v "$tool" >/dev/null; then printf '[OK] %s\n' "$tool";
    else printf '[Opzionale, non rilevato] %s\n' "$tool"; fi
done
confirm() { local reply; read -r -p "$1 [s/N] " reply; [[ "$reply" == s || "$reply" == S ]]; }
finish() {
    if [[ "$check_only" != --check ]]; then
        source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/scripts/prepare_codex.sh"
        if ! prepare_codex; then echo 'Preparazione Codex non riuscita; Oaclis può proseguire senza Codex.'; fi
    fi
    if [[ "$check_only" == --check ]]; then
        echo 'Prerequisiti Oaclis presenti. Oaclis non è stato compilato o avviato.'
    else
        bash "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/scripts/install.sh" --unconfigured
        if [[ "$wsl" != yes ]] && ! "$HOME/.local/bin/oaclis" prepare-awake; then
            echo 'Sempre attivo non preparato; Oaclis viene installato comunque. Riprovare con bash run.sh --prepare-awake.'
        fi
        "$HOME/.local/bin/oaclis" save-scenario "$system" "$container" "$wsl"
        echo 'Oaclis compilato e installato. Cerco il master nella LAN sulla 23232.'
        "$HOME/.local/bin/oaclis" setup
        if [[ "$system" == Linux && "$container" == no ]]; then
            "$HOME/.local/bin/oaclis" prepare-service
            if [[ "$wsl" != yes ]]; then
                "$HOME/.local/bin/oaclis" open-ui
            fi
        else
            echo 'Avvio automatico non configurato su macOS/container: avviare Oaclis con la gestione del sistema ospitante.'
        fi
    fi
}
if [[ ${#missing[@]} -eq 0 ]]; then finish; exit 0; fi
if [[ "$check_only" == --check ]]; then exit 1; fi
if [[ ! -t 0 ]]; then echo 'Per installare serve un terminale interattivo. Nessuna modifica eseguita.' >&2; exit 1; fi
if [[ "$system" == Darwin ]] && { ! command -v cc >/dev/null || ! xcode-select -p >/dev/null 2>&1; }; then
    echo 'Installare prima gli strumenti Apple con: xcode-select --install'
    exit 1
fi
if [[ ${#packages[@]} -gt 0 ]]; then
    printf 'Pacchetti mancanti: %s\n' "${packages[*]}"
    if ! confirm 'Installare questi pacchetti?'; then echo 'Installazione annullata.'; exit 1; fi
    if [[ "$system" == Linux ]]; then
        command -v apt-get >/dev/null || { echo 'Installazione automatica disponibile solo per Ubuntu/Debian. Nessuna modifica eseguita.' >&2; exit 1; }
        elevate=()
        if (( EUID != 0 )); then
            command -v sudo >/dev/null || { echo 'Serve sudo per installare i pacchetti di sistema.' >&2; exit 1; }
            elevate=(sudo)
        fi
        "${elevate[@]}" apt-get update
        "${elevate[@]}" apt-get install -y "${packages[@]}" ca-certificates
    else
        command -v brew >/dev/null || { echo 'Homebrew assente: installarlo dal sito ufficiale https://brew.sh e ripetere.' >&2; exit 1; }
        if ! command -v cc >/dev/null; then echo 'Installare prima gli strumenti Apple con: xcode-select --install'; exit 1; fi
        brew install "${packages[@]}"
    fi
fi
if ! command -v cargo >/dev/null || ! command -v rustc >/dev/null || [[ " ${missing[*]} " == *' rustc '* ]]; then
    if ! confirm 'Installare/aggiornare Rust stable con rustup ufficiale?'; then echo 'Rust non installato/aggiornato.'; exit 1; fi
    if command -v rustup >/dev/null; then rustup update stable; rustup default stable;
    else
        rust_setup=$(mktemp)
        trap 'test ! -f "$rust_setup" || unlink "$rust_setup"' EXIT
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "$rust_setup"
        sh "$rust_setup" -y --default-toolchain stable
    fi
fi
bash "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/run.sh" --check
finish
