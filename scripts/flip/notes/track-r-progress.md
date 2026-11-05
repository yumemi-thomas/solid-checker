# Track R progress

Branch `claude/flip-reads`. One line per rule; fresh agents continue from here.

Helpers (this machine, scratch): `/tmp/sc-scratch.noindex/flip-reads`: `ff.sh <name> <bindir> <codes> --grids ...` (focused run), `sw.sh <tag> <prev>` (sweep build `r<tag>`, compare), `redisc.py` (append a rediscovery entry), `base/` (build at the branch cut), `sweep-base.json` (16,036 U, 383 V).

| Rule | State | Corpus move | Notes |
|---|---|---|---|
| SC5007 | done, ADR 0650 | 0 | old grid 9,540 false cleans, new 0; also a possibly undefined accessor is no longer a violation |
| SC8003 | done, ADR 0651 | +224 U, 0 V moved | witnesses 8 of 8; every added site is an open spread on an intrinsic; `outside` in rediscovery: no runtime code; also drops the old false V on `innerHTML` beside a blank child |
| SC1005 | done, ADR 0652 | +1 U (`!user` from `useContext`), 0 V moved | witnesses 9 of 9; `outside`: no runtime code |
| SC1007 | done, ADR 0653 | +41 U, 0 V moved | witnesses 3 of 3; every site a handler read from a hook's result; `outside`: the grid judges the site statement, not an attribute. Follow-up proof: a project hook returning a closed literal of functions is a plain object, which should recover most of the 41 |
| SC2007 | done, ADR 0654 | 0 | witnesses 9 of 9; old grid 1,500 false cleans, new 0; adds `return_open` (protocol 89), which SC7002 and SC5003 read |
| SC7002 | done, ADR 0655 | +1 U (`createSignal(load, {...options})`, generic return), 0 V moved | witnesses 7 of 7; old grid 1,140 false cleans, new 0 |

Next: SC1004, SC1003, then `async.md`'s order for SC5008, SC5006, SC5003+SC5001, which also owns `lazy`, `cases3-p03`, and `patterns.mjs:130` first. Latest sweep: `sweep-7002.json`, 16,303 U, 383 V.

`sc1001.md`, `f0.md`, `sc9012-census.md` beat the plan: reread the row's section before each rule. They add to rules in between: SC5003+SC5001 owns `dynamic_component_entry`, the `<Dynamic>` hint, check `cases3-p03` there, and maybe C-D4 (`n01 n03 n06`); SC1001 has 10 steps and needs `patterns.mjs:130`; H-D5 lands before the census; `check.mjs` needs SC9012 for 9 ids (W's 8 and `o12`) in the census commit.

Surprises:
- Chain gate and commit with `&&` or an explicit test: a pipeline hid a failed `ownership-gate` once, and the commit had to be amended before pushing.
- `ef46d0ac6` and `aaaf31cad`: verify-delta rows for rule pages, `.gitignore` and `scripts/flip/`. A progress-file edit now stays in the fast plan.
- `ef46d0ac6` (Track W too): verify-delta maps `docs/rules/` and `.gitignore`. Its message says 27 of 27 tests; it was 26 of 26. Pushed, so not amended.
- `type_descriptor_spans` also turns on callability, and a descriptor is evidence other consumers read: demanding one on a coerced operand moved an unrelated SC9012. Use `callability_only_spans` when only callability is wanted.
- `ir-read-lint` matches by name: `.functions.iter()` in a file that declares a `functions` map counts as a hash iteration.
- `diagnostics_process.rs` pins per-fixture finding counts: extending a fixture moves them. `make coverage` does not show this; `nt.sh` does.
- `ce6a831b2` and `ae2327986` are base repairs for Track W: 3 process tests, the read ratchet, WASM parity. All left behind by deliberate changes; `make verify` had never run on the base.
- `cases.json` and `rediscovery.json` are append-only: edit by text. `json.dumps` reformats the whole file and makes a certain merge conflict worse.
- The branch cut had 3 red Rust tests (none ran `make verify` there); fixed in the commit `test(flip-base): ...`, for Track W to pick up. Rust tests are 1,252 of 1,252 from then on; `nt.sh` prints anything new. Pass `BASE=<last rule commit>` to verify-delta, or it maps thousands of paths against `origin/main`.
- `scripts/fuzz/install` needs `npm ci` too (Chrome host); the plan names only the oracle install.
- `view` children in a generator context must be numbers: build a boundary in `body` with `domExpr`.
