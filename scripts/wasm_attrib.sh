#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later

# Where the reference scatter's WASM bytes go (GUP-450). Builds the scatter
# harness as scripts/wasm_size.sh does, binds it with `wasm-bindgen --target
# web` keeping the name section, and prints three sections from `twiggy`'s
# shallow sizes (raw bytes, not gz): `== top`, `== monos` and `== crates`. The
# name section is not shipped, so it is reported apart and left out of the
# crate totals. Local diagnostic only; judge a cut by `mask wasm-size`.
#
# Needs: twiggy (the dev shell), and what wasm_size.sh needs.

set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
harness="$root/crates/gup-core/wasm-size/scatter"
target="${CARGO_TARGET_DIR:-target}"
[[ $target == /* ]] || target="$root/$target"
export CARGO_TARGET_DIR="$target"

command -v twiggy >/dev/null || {
  echo "wasm-attrib: needs twiggy (it is in the dev shell: nix develop)" >&2
  exit 1
}
want=$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;}' "$harness/Cargo.lock")
have=$(wasm-bindgen --version 2>/dev/null | cut -d' ' -f2 || true)
if [[ $have != "$want" ]]; then
  echo "wasm-attrib: needs wasm-bindgen $want, found '${have:-none}'" >&2
  exit 1
fi

out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
(cd "$harness" && cargo build --quiet --release --target wasm32-unknown-unknown)
wasm-bindgen --target web --out-dir "$out" --out-name scatter \
  "$target/wasm32-unknown-unknown/release/gup_core_wasm_size_scatter.wasm"
twiggy top -n 1000000 -f csv "$out/scatter_bg.wasm" >"$out/items.csv"

python3 -I - "$out/items.csv" "$(stat -c %s "$out/scatter_bg.wasm")" <<'PY'
import csv, re, sys
from collections import defaultdict

rows = [(r[0], int(r[1])) for r in list(csv.reader(open(sys.argv[1])))[1:]]
total = int(sys.argv[2])
names = sum(s for n, s in rows if n == "\"function names\" subsection")
items = [(n, s) for n, s in rows if n != "\"function names\" subsection"]
pct = lambda s: 100.0 * s / total
hash_re = re.compile(r"::h[0-9a-f]{16}$")
crate_re = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*(?=::)")

def crate(name):
    if name.startswith("data segment"):
        return "data"
    m = crate_re.match(name.lstrip("<"))
    return m.group(0) if m else "other"

print(f"file {total} B raw; name section {names} B (not shipped, excluded below)")
print("\n== top")
for n, s in sorted(items, key=lambda i: -i[1])[:30]:
    print(f"{s:9d} B {pct(s):5.2f}%  {n}")

groups = defaultdict(lambda: [0, 0])
for n, s in items:
    if hash_re.search(n):
        g = groups[hash_re.sub("", n)]
        g[0] += 1
        g[1] += s
print("\n== monos")
print(f"{'total B':>9} {'count':>5}  group")
multi = [(k, v) for k, v in groups.items() if v[0] > 1]
for k, (c, s) in sorted(multi, key=lambda i: -i[1][1])[:20]:
    print(f"{s:9d} {c:5d}  {k}")

crates = defaultdict(int)
for n, s in items:
    crates[crate(n)] += s
print("\n== crates")
for k, s in sorted(crates.items(), key=lambda i: -i[1]):
    print(f"{s:9d} B {pct(s):5.2f}%  {k}")
PY
