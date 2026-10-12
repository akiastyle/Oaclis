#!/usr/bin/env bash
set -euo pipefail
role=${1:-client}
case "$role" in client|master) ;; *) echo 'Uso: bash scripts/install.sh [client|master]' >&2; exit 2;; esac
[[ $(uname -s) == Linux ]] || { echo 'Questo installer supporta Linux e WSL.' >&2; exit 1; }
source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
export PATH="$HOME/.cargo/bin:$PATH"
for command in cargo python3 ssh ssh-keygen rsync; do
    command -v "$command" >/dev/null || { echo "Dipendenza mancante: $command. Consulta README.md." >&2; exit 1; }
done
cargo build --release --locked --manifest-path "$source_dir/Cargo.toml"
install -d "$HOME/.local/bin" "$HOME/.config/oaclis"
install -m 755 "$source_dir/target/release/oaclis" "$HOME/.local/bin/oaclis.new"
mv -f "$HOME/.local/bin/oaclis.new" "$HOME/.local/bin/oaclis"
# Non cambiare il ruolo di un'installazione già configurata.
if [[ ! -e "$HOME/.config/oaclis/role" ]]; then
    printf '%s\n' "$role" > "$HOME/.config/oaclis/role"
fi
printf 'Installato: %s/.local/bin/oaclis\nRuolo: ' "$HOME"
cat "$HOME/.config/oaclis/role"
printf 'Avvio: %s/.local/bin/oaclis serve\nUI: http://localhost:23232\n' "$HOME"
for command in tmux codex; do
    command -v "$command" >/dev/null || printf 'Opzionale per aprire le chat: %s\n' "$command"
done
