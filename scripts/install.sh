#!/usr/bin/env bash
# install.sh v1.0.26
set -euo pipefail
role=${1:-client}
case "$role" in client|master|--unconfigured) ;; *) echo 'Uso: bash scripts/install.sh [client|master|--unconfigured]' >&2; exit 2;; esac
case $(uname -s) in Linux|Darwin) ;; *) echo 'Questo installer supporta Linux, WSL e macOS.' >&2; exit 1;; esac
source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
export PATH="$HOME/.cargo/bin:$PATH"
for command in cargo python3 ssh ssh-keygen rsync; do
    command -v "$command" >/dev/null || { echo "Dipendenza mancante: $command. Consulta README.md." >&2; exit 1; }
done
cargo build --release --locked --manifest-path "$source_dir/Cargo.toml"
install -d "$HOME/.local/bin" "$HOME/.config/oaclis"
install -d -m 700 "$HOME/.local/share/oaclis"
install -m 600 "$source_dir/tools/keep_awake.py" "$HOME/.local/share/oaclis/keep_awake.py"
install -m 755 "$source_dir/target/release/oaclis" "$HOME/.local/bin/oaclis.new"
mv -f "$HOME/.local/bin/oaclis.new" "$HOME/.local/bin/oaclis"
# Non cambiare il ruolo di un'installazione già configurata.
if [[ "$role" != --unconfigured && ! -e "$HOME/.config/oaclis/role" ]]; then
    printf '%s\n' "$role" > "$HOME/.config/oaclis/role"
fi
printf 'Installato: %s/.local/bin/oaclis\nRuolo: ' "$HOME"
if [[ -e "$HOME/.config/oaclis/role" ]]; then cat "$HOME/.config/oaclis/role"; else echo 'da configurare'; fi
printf 'Avvio: %s/.local/bin/oaclis serve\nUI: http://localhost:23232\n' "$HOME"
for command in tmux codex; do
    command -v "$command" >/dev/null || printf 'Opzionale per aprire le chat: %s\n' "$command"
done
