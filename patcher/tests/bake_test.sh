#!/usr/bin/env bash
# Bakes build/dist patchsets onto pristine exes and compares against the sha1s listed in
# <expected_sha1.txt> ("Exe.exe <sha1>" per line). Pristine exes are looked up in $SC_CLOUD under their
# own name or any *.bak_* copy whose sha1 matches the patchset (they are not in the repo).
set -euo pipefail
B=${1:-build}
EXPECTED=${2:?usage: bake_test.sh <build-dir> <expected_sha1.txt>}
: "${SC_CLOUD:?set SC_CLOUD to the game cloud/ directory}"
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
fail=0

sha() { sha1sum "$1" | awk '{print $1}'; }

while read -r exe want; do
  case "$exe" in ''|'#'*) continue ;; esac
  ps="$B/dist/$exe.patchset.toml"
  pristine_sha=$(grep -m1 '^sha1 = ' "$ps" | tr -dc '0-9a-f' | tail -c 40)
  src=""
  for f in "$SC_CLOUD/$exe" "$SC_CLOUD/$exe".bak_*; do
    [ -f "$f" ] || continue
    h=$(sha "$f")
    if [ "$h" = "$pristine_sha" ]; then src=$f; break; fi
  done
  if [ -z "$src" ]; then
    echo "SKIP $exe: no pristine copy with sha1 $pristine_sha in $SC_CLOUD"
    continue
  fi
  "$B/sfp" bake "$ps" "$src" -o "$out/$exe" >/dev/null
  got=$(sha "$out/$exe")
  if [ "$got" = "$want" ]; then
    echo "PASS $exe -> $got"
  else
    echo "FAIL $exe: baked $got, expected $want"
    fail=1
  fi
  if ! "$B/sfp" verify "$ps" "$src" >/dev/null; then
    echo "FAIL $exe: verify on pristine"
    fail=1
  fi
done < "$EXPECTED"
exit $fail
