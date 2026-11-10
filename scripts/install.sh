#!/usr/bin/env bash
# install.sh v1.0.51
set -euo pipefail
role=${1:-client}
case "$role" in client|master|--unconfigured) ;; *) echo 'Uso: bash scripts/install.sh [client|master|--unconfigured]' >&2; exit 2;; esac
case $(uname -s) in Linux|Darwin) ;; *) echo 'Questo installer supporta Linux, WSL e macOS.' >&2; exit 1;; esac
source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
export PATH="$HOME/.cargo/bin:$PATH"
for command in cargo ssh ssh-keygen rsync; do
    command -v "$command" >/dev/null || { echo "Dipendenza mancante: $command. Consulta README.md." >&2; exit 1; }
done
cargo build --release --locked --manifest-path "$source_dir/Cargo.toml"
install -d "$HOME/.local/bin" "$HOME/.config/oaclis"
install -m 755 "$source_dir/target/release/oaclis" "$HOME/.local/bin/oaclis.new"
mv -f "$HOME/.local/bin/oaclis.new" "$HOME/.local/bin/oaclis"
"$HOME/.local/bin/oaclis" >/dev/null
# Remove only the unmodified, obsolete Oaclis helper; never remove project Python files.
legacy_awake="$HOME/.local/share/oaclis/keep_awake.py"
if [[ -f "$legacy_awake" && ! -L "$legacy_awake" && ! -L "$HOME/.local/share/oaclis" ]]; then
    legacy_hash=''
    if command -v sha256sum >/dev/null; then legacy_hash=$(sha256sum "$legacy_awake");
    elif command -v shasum >/dev/null; then legacy_hash=$(shasum -a 256 "$legacy_awake"); fi
    if [[ "${legacy_hash%% *}" == d2db2275aa1fa11d889ead1c0a3c0bfc12377ab314c8c30f1586d0d685e322c4 ]]; then
        unlink "$legacy_awake"
    else
        echo 'Helper standby precedente personalizzato o non verificabile: conservato.'
    fi
fi
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
