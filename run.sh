#!/usr/bin/env bash
# Prepare prerequisites only: compilation and application startup are separate steps.
set -euo pipefail
case "${1:-}" in ''|--check) ;; *) echo 'Uso: bash run.sh [--check]' >&2; exit 2;; esac
check_only=${1:-}
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
save_scenario() {
    [[ "$check_only" != --check ]] || return 0
    if command -v python3 >/dev/null; then
        python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/scripts/save_settings.py" "$system" "$container" "$wsl"
    else
        echo 'Scenario non ancora salvato: serve Python 3.'
    fi
}
missing=(); packages=()
check() {
    if command -v "$1" >/dev/null; then printf '[OK] %s\n' "$1";
    else printf '[Manca] %s\n' "$1"; missing+=("$1"); [[ -z "$2" ]] || packages+=("$2"); fi
}
if [[ "$system" == Linux ]]; then
    for item in git:git curl:curl python3:python3 ssh:openssh-client ssh-keygen:openssh-client rsync:rsync tmux:tmux cc:build-essential ip:iproute2; do check "${item%%:*}" "${item#*:}"; done
else
    for item in git:git curl:curl python3:python ssh:openssh ssh-keygen:openssh rsync:rsync tmux:tmux cc:; do check "${item%%:*}" "${item#*:}"; done
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
    save_scenario
    echo 'Prerequisiti Oaclis presenti. Oaclis non è stato compilato o avviato.'
}
if [[ ${#missing[@]} -eq 0 ]]; then finish; exit 0; fi
if [[ "$check_only" == --check ]]; then exit 1; fi
save_scenario
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
        elevate=(); if (( EUID != 0 )); then command -v sudo >/dev/null || { echo 'Serve root o sudo.' >&2; exit 1; }; elevate=(sudo); fi
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
finish
bash "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/run.sh" --check
