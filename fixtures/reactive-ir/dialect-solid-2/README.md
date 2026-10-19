# dialect-solid-2

Half of a retired pair (the 1.x half is gone): the package declaration preserves the real 2.0.0-rc.0 `createEffect` overloads and `node_modules/solid-js/package.json` selects 2.0. ADR 0027 uses it in `contracts_process`: native findings are identical with no catalog and with obsolete core catalog entries whose documents do not exist; `--check-contracts` reports `solid-js` as `builtin`, never independently certified or needing a core receipt. The stub selects the reviewed dialect; it does not authenticate installed runtime bytes.
