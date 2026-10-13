# Probes for the 2026-09-30 rc.9 `reads` rows

Evidence scripts for `../2026-09-30-solid-2-rc9-reads-rows-for-dialect-primitives.md`,
not part of any gate. They read the provisioned audited install
(`make audited-archives-provision`, `rust/target/audited-archives/solid-v2/2.0.0-rc.9/node_modules`).

- `callgraph.mjs <package-dir> <prod|observe|dev> <root>...` — the read-site call
  graph (`CUT=flush`, `VERBOSE=1`, `WHY=<fn>` are honoured). It needs `acorn`
  (`packages/cli/node_modules`).
- `probe.mjs` — ADR 0163-style probe of the signals primitives. Run it with a
  `node_modules` symlink to the audited install beside it:
  `PROBE_BUILD=prod node probe.mjs`,
  `PROBE_BUILD=dev node --conditions=development probe.mjs`,
  `PROBE_BUILD=observe node --conditions=observe probe.mjs`.
- `probe-solid-js.mjs` — the same for `solid-js`' `createSignal` and `useContext`:
  `PROBE_BUILD=client-prod node --conditions=browser probe-solid-js.mjs`, and
  `--conditions=browser --conditions=development` / `--conditions=observe`;
  `PROBE_BUILD=server-prod node --conditions=node probe-solid-js.mjs` and the
  `development` / `observe` twins. Conditions are repeated flags, not a list.
