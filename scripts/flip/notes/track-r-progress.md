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
| SC1004 | done, ADR 0656 | -30 SC1001 U, +29 SC1004 U, same sites, 0 V moved | witnesses 2 of 2; `outside` |
| SC1003 | done, ADR 0657 | +129 U, 0 V moved | witnesses 9 of 10, `o12` waits for the census as the plan says; old grid 144 false cleans, new 0; 3 V-D5 patterns left the open list, 390 open |

| SC5008 | done, ADR 0658 | +179 U, 0 V moved | witnesses 4 of 4; old grid 12 false cleans, new 0 |
| Shared | ADR 0659, `40bf35ae9`, `cee52c939` | 0 (sweep runs `csr`) | `ExecutionPaths`, `--rendering hydrated`, host witnesses, `judgeSsr` |
| SC5006 | done, ADR 0660 | 0 | witnesses 5 of 5; `outside` |
| SC5003 gate | ADR 0661, `async.md` steps 1, 2 | 0 | `e02` holds; `j02 j03 j06` wait for steps 6 and 8 |
| `<Dynamic>` | ADR 0662, `async.md` steps 3, 5 | 0 | `p-dyn` holds; `m1` matrix: 0 false positives on the note's mounts; 8 patterns left the open list, 381 open |

| Merge | `059beb67a` from Track W, the first merge | 0 moved by the merge | `make verify` green, `check.mjs all` 181 of 335 (no keep, regress or warn fails), HOST 20 of 24, patterns 381 open |
| Member types | ADR 0663 | +17 U, all SC9012, 0 V moved | ViewerToolbar explained: a member's type answered for its receiver. `outside`, program in `patterns/values/sibling-type-go` |
| `clientOnly`, `lazy` + SSR | ADR 0664, `async.md` step 4 | see below | `ExecutionPaths` `DeferredMount` arms; HOST 22 of 25, 3 `NoHydration` rows left; 4 patterns left the open list, 377 open |
| Open provenance, `lazy`, method reads, cover | ADRs 0665-0668, `async.md` steps 6-8, fuzz contexts | -4 SC1001 V, +687 U (434 SC5003, 128 SC5001, 125 SC9012), `sweep-6.json`: 17,312 U, 379 V | SC5003 5 of 5; 8 patterns left, 369 open; 547 of the U are inferay, whose `@contracts` points at a missing build, so its types are `any`. The 4 V are `useSearch` in `probus-hk`, tell the owner |

Next, owner's order: (1) done, ADR 0659, `ExecutionPaths`, `--rendering hydrated`; (2) done, `scripts/flip/host-witnesses.json`, `check.mjs HOST` 20 of 24, the other 4 marked `open`; `judgeSsr` reads every client rule; (3) SC5003+SC5001 by `async.md`, steps 0-5 done, next 6, 7, 8 and the fuzz contexts (`dyn-jsx-*`, `portal-*`, `*-later`, `BODY_LABEL`, `<Portal>` in a kept grid); (4) SC1002, SC8014, SC8015, SC1001 (add the arm fact and a `NoHydration` blocker: `unplaced_run` misses D1, D3, because both are `MayRun`), SC9012. Latest sweep: `sweep-co.json`, 16,652 U, 383 V (the merge added 25, W's ADR 0705; ADR 0663 added 17).

`sc1001.md`, `f0.md`, `sc9012-census.md` beat the plan: reread the row's section before each rule. They add to rules in between: SC5003+SC5001 owns `dynamic_component_entry`, the `<Dynamic>` hint, check `cases3-p03` there, and maybe C-D4 (`n01 n03 n06`); SC1001 has 10 steps and needs `patterns.mjs:130`; H-D5 lands before the census; `check.mjs` needs SC9012 for 9 ids (W's 8 and `o12`) in the census commit.

Open for the census: `const hook = c ? useThing : useThing; const t = hook(); createMemo(() => { t.go(1); ... })` is certified and Chrome throws (`research/probes/v1/a`, `v2/a-nosib`). A hook picked by a conditional reaches no `handed_value` arm. Not a sibling-type case. Close it with F0 or the census, and add it to the fuzzer there.

Next: SC1002, SC8014, SC8015, then SC1001, H-D5, F0, SC9012. Latest sweep `sweep-6.json`, build `r6/`. Helpers: `bld.sh <tag>`, `f.sh <tag> [dir]` (one fixture), `tab.py`, `diffg.py <old> <new>` (grid outcome moves), `snapd.py` (moved snapshot rows). Open, all on the build before step 6 too: mx-src `s-async-value-member` 48 and mx-combo 23 pending false cleans; mx-recall `store-dstr-*` in `ref*` and applies 596 (ADR 0657 did not reach them); `open-defects.json`'s partial grid entry holds 690 ASYNC_OUTSIDE false cleans for the census. A parameter compute (`createMemo(options)`) is open: a fact for a value's signature would close it.

Handoff at ADR 0664: helpers `chk.sh` (scratch, oracle path fixed), `rd.py <cmp> <sweep> x` (each added site with its source line), `rm/` (merge build), `r9012/`, `rco/`. Do first: step 6 (`computation_provenance`, four discovery sites, wording flag, C-D4), then 7 (`lazy` tag row), 8 (`j06`), the fuzz contexts. Step 6 reaches W: an `any`-computed `createOptimistic` starts to hold, tell W in the commit.

Surprises:
- A recall-only code's own cells go in `recallContexts`, with `contexts: []`, or `recall.test.mjs` counts them as base cells.
- `MISUSE_CODES` left out advisory codes, so SC5003 never had a grid until ADR 0668.
- ADR 0659 limits to carry: a site in a helper only an arm calls keeps its finding; dependency source is the client graph on both paths; the sweep still runs `csr`.
- Chain gate and commit with `&&` or an explicit test: a pipeline hid a failed `ownership-gate` once, and the commit had to be amended before pushing.
- `ef46d0ac6` and `aaaf31cad`: verify-delta rows for rule pages, `.gitignore` and `scripts/flip/`. A progress-file edit now stays in the fast plan.
- `ef46d0ac6` (Track W too): verify-delta maps `docs/rules/` and `.gitignore`. Its message says 27 of 27 tests; it was 26 of 26. Pushed, so not amended.
- `type_descriptor_spans` also turns on callability, and a descriptor is evidence other consumers read: demanding one on a coerced operand moved an unrelated SC9012. Use `callability_only_spans` when only callability is wanted.
- `ir-read-lint` matches by name: `.functions.iter()` in a file that declares a `functions` map counts as a hash iteration.
- `diagnostics_process.rs` pins per-fixture finding counts: extending a fixture moves them. `make coverage` does not show this; `nt.sh` does.
- `make verify` selects `repro-causes`, `verify-delta` does not for a rule change. Run it when a rule moves or rewords a finding: `fixtures/uncertifiable-repro/cases.json` pins causes, and 5 had gone stale before the first merge. W's messages too: `uncertifiable-causes.mjs` matches wording.
- A generator context can only wrap `stmts`: a member that must itself be the setter is outside it. Keep such a program in `scripts/fuzz/patterns/`, where the old build fails it as `NEW false-clean`.
- A JSX element in a context's `body` is created, not rendered. `children(() => <X />)()` forces it.
- `ce6a831b2` and `ae2327986` are base repairs for Track W: 3 process tests, the read ratchet, WASM parity. All left behind by deliberate changes; `make verify` had never run on the base.
- `cases.json` and `rediscovery.json` are append-only: edit by text. `json.dumps` reformats the whole file and makes a certain merge conflict worse.
- The branch cut had 3 red Rust tests (none ran `make verify` there); fixed in the commit `test(flip-base): ...`, for Track W to pick up. Rust tests are 1,252 of 1,252 from then on; `nt.sh` prints anything new. Pass `BASE=<last rule commit>` to verify-delta, or it maps thousands of paths against `origin/main`.
- `scripts/fuzz/install` needs `npm ci` too (Chrome host); the plan names only the oracle install.
- `view` children in a generator context must be numbers: build a boundary in `body` with `domExpr`.
