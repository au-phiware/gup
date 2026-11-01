# Test fonts

`NotoSansJP-Test.ttf` (about 2.6 KB) is a subset of Noto Sans JP Regular holding
only "東京" (and `.notdef`), used by the fallback-chain tests. It is not bundled
in the crate; tests `include_bytes!` it. Its licence is the SIL Open Font
License 1.1, in `NotoSansJP-OFL.txt` next to it.

Regenerate it with fonttools from the dev shell (the Noto Sans JP variable font
comes from nixpkgs' `google-fonts`; find it with `fc-list | grep NotoSansJP`):

```bash
fonttools varLib.instancer --update-name-table -o target/NotoSansJP-Regular.ttf "<path>/NotoSansJP[wght].ttf" wght=400
pyftsubset target/NotoSansJP-Regular.ttf --text="東京" --layout-features=kern \
  --no-hinting --notdef-outline --name-IDs=0,1,2,3,4,5,6,7,13,14 \
  --output-file=crates/gup-text/tests/fonts/NotoSansJP-Test.ttf
```
