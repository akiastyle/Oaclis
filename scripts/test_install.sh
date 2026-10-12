#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf "$test_dir"' EXIT
mkdir -p "$test_dir/repo/scripts" "$test_dir/repo/target/release" "$test_dir/home" "$test_dir/bin"
cp "$repo/scripts/install.sh" "$test_dir/repo/scripts/"
printf '#!/bin/sh\nexit 0\n' > "$test_dir/repo/target/release/oaclis"
for tool in cargo python3 ssh rsync; do
    printf '#!/bin/sh\nexit 0\n' > "$test_dir/bin/$tool"
    chmod +x "$test_dir/bin/$tool"
done
export HOME="$test_dir/home" PATH="$test_dir/bin:$PATH"
bash "$test_dir/repo/scripts/install.sh" client >/dev/null
[[ $(cat "$HOME/.config/oaclis/role") == client ]]
[[ -x "$HOME/.local/bin/oaclis" ]]
printf 'preserve' > "$HOME/.config/oaclis/example"
bash "$test_dir/repo/scripts/install.sh" master >/dev/null
[[ $(cat "$HOME/.config/oaclis/role") == client ]]
[[ $(cat "$HOME/.config/oaclis/example") == preserve ]]
if bash "$test_dir/repo/scripts/install.sh" invalid >/dev/null 2>&1; then exit 1; fi
printf 'Installer: prima installazione, aggiornamento e validazione OK\n'
