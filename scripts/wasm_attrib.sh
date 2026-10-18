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
# `--row-type` (GUP-459) instead builds the harness with and without its
# `extra-row-type` feature and prints `== row type`: the per-group raw byte and
# instantiation-count differences (hashes and `.NNNN` suffixes stripped), the 30
# largest by absolute size, and the total. That is what one more `Selection` row
# type costs, item by item; `wasm.per_row_type.gz_bytes` is its gz total.
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

row_type=
case "${1:-}" in
  "") ;;
  --row-type) row_type=1 ;;
  *)
    echo "usage: wasm_attrib.sh [--row-type]" >&2
    exit 2
    ;;
esac

out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
bind() { # output name, extra cargo args
  local name=$1
  shift
  (cd "$harness" && cargo build --quiet --release --target wasm32-unknown-unknown "$@")
  wasm-bindgen --target web --out-dir "$out" --out-name "$name" \
    "$target/wasm32-unknown-unknown/release/gup_core_wasm_size_scatter.wasm"
  twiggy top -n 1000000 -f csv "$out/${name}_bg.wasm" >"$out/$name.csv"
}

if [[ -n $row_type ]]; then
  bind with --features extra-row-type
  bind without
  python3 -I - "$out/with.csv" "$out/without.csv" <<'PY'
import csv, re, sys
from collections import defaultdict

hash_re = re.compile(r"::h[0-9a-f]{16}(\.\d+)?$")
# Imports are named after the file wasm-bindgen wrote, and may end in a hash.
import_re = re.compile(r"\./(?:with|without)_bg\.js::(.*?)(?:_[0-9a-f]{16})?$")
mid_re = re.compile(r"::h[0-9a-f]{16}(?=\s)")

def groups(path):
    g = defaultdict(lambda: [0, 0])
    for r in list(csv.reader(open(path)))[1:]:
        if r[0] == "\"function names\" subsection":
            continue
        e = g[mid_re.sub("", import_re.sub(r"import \1", hash_re.sub("", r[0])))]
        e[0] += 1
        e[1] += int(r[1])
    return g

with_t, without = groups(sys.argv[1]), groups(sys.argv[2])
diff = []
for k in set(with_t) | set(without):
    c = with_t[k][0] - without[k][0] if k in without else with_t[k][0]
    b = with_t[k][1] - (without[k][1] if k in without else 0)
    if k in without and k not in with_t:
        c, b = -without[k][0], -without[k][1]
    if b or c:
        diff.append((k, c, b))
print("== row type")
print(f"{'raw B':>8} {'count':>6}  group")
for k, c, b in sorted(diff, key=lambda i: -abs(i[2]))[:30]:
    print(f"{b:+8d} {c:+6d}  {k}")
print(f"{sum(b for _, _, b in diff):+8d} {sum(c for _, c, _ in diff):+6d}  total (raw, outside the name section)")
PY
  exit 0
fi

bind scatter
mv "$out/scatter.csv" "$out/items.csv"

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
