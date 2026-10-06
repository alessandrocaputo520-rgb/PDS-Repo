#!/bin/sh
# Eseguire dalla cartella G10, dopo cargo build --release --workspace.
set -eu
for f in target/release/server target/release/client; do
  [ -f "$f" ] || { printf 'Manca %s (eseguire cargo build --release --workspace)\n' "$f"; exit 1; }
  wc -c < "$f" | awk -v name="$f" '{ printf "%s: %s byte (%.2f MiB)\n", name, $1, $1/1048576 }'
done
