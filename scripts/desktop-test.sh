#!/usr/bin/env bash
# desktop-test.sh v1.0.53
# Isolated attended desktop trial; no Oaclis state or network configuration changes.
set -euo pipefail
action=${1:---check}
case "$action" in --check|--prepare) ;; *) echo 'Uso: bash scripts/desktop-test.sh [--check|--prepare]' >&2; exit 2;; esac
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
system=$(uname -s)
if [[ "$system" == Linux && ( -n ${WSL_DISTRO_NAME:-} || $(uname -r) == *[Mm]icrosoft* ) ]]; then
    powershell=/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe
    [[ -x "$powershell" ]] || { echo 'PowerShell Windows non disponibile.' >&2; exit 1; }
    windows_script=$(wslpath -w "$script_dir/desktop-test.ps1")
    mode=Check; [[ "$action" != --prepare ]] || mode=Prepare
    exec "$powershell" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$windows_script" -Action "$mode"
fi
case "$system" in
    Linux)
        # WSLg is handled above. On native Linux require an actual graphical login.
        graphical=no
        if command -v loginctl >/dev/null; then
            while read -r session _; do
                [[ -n "$session" ]] || continue
                type=$(loginctl show-session "$session" -p Type --value 2>/dev/null || true)
                case "$type" in x11|wayland) graphical=yes;; esac
            done < <(loginctl list-sessions --no-legend 2>/dev/null || true)
        fi
        if [[ "$graphical" != yes ]]; then echo 'Nessun desktop Linux: resta disponibile la console.'; exit 0; fi
        if command -v rustdesk >/dev/null; then echo 'RustDesk presente sul desktop Linux.'; exit 0; fi
        [[ "$action" != --check ]] || { echo 'RustDesk da installare sul desktop Linux.'; exit 1; }
        command -v apt-get >/dev/null || { echo 'Per questo sistema installare RustDesk dal sito ufficiale.' >&2; exit 1; }
        case $(uname -m) in
            x86_64) arch=x86_64; digest=4e2b9701d25a7ef373f3d09bda584a37d6902f97a6ab15f2e86d4a5783e2a0ec;;
            aarch64) arch=aarch64; digest=1f2fc9be8321c10347aa33cb13198c6a956aec272c5e58ac81f4c0ef04037df3;;
            *) echo 'Architettura non supportata dal test automatico.' >&2; exit 1;;
        esac
        package_dir=$(mktemp -d)
        trap 'rm -rf -- "$package_dir"' EXIT
        package="$package_dir/rustdesk.deb"
        curl --proto '=https' --tlsv1.2 -fL --connect-timeout 10 --max-time 180 \
            "https://github.com/rustdesk/rustdesk/releases/download/1.5.0/rustdesk-1.5.0-$arch.deb" -o "$package"
        printf '%s  %s\n' "$digest" "$package" | sha256sum -c -
        elevate=(); if (( EUID != 0 )); then elevate=(sudo); fi
        "${elevate[@]}" apt-get install -y "$package"
        echo 'RustDesk installato. Aprirlo nel desktop Linux e approvare la connessione di prova.'
        ;;
    Darwin)
        if [[ -d /Applications/RustDesk.app || -d "$HOME/Applications/RustDesk.app" ]]; then echo 'RustDesk presente sul Mac.'; exit 0; fi
        [[ "$action" != --check ]] || { echo 'RustDesk da installare sul Mac.'; exit 1; }
        command -v brew >/dev/null || { echo 'Homebrew assente: installare RustDesk dal sito ufficiale.' >&2; exit 1; }
        brew install --cask rustdesk
        echo 'RustDesk installato. Autorizzare Registrazione schermo e Accessibilita nelle impostazioni del Mac.'
        ;;
    *) echo 'Sistema non supportato.' >&2; exit 1;;
esac
