# A helper re-exported and called by the same entry file joins to both names

`index.js` does two things with one declaration: it re-exports it
(`export { channelFor } from "./channel.js"`) and it calls it
(`import { channelFor } from "./channel.js"`, then `channelFor(props.client)`
inside `forwarded`). The specifier and the local binding are separate compiler
symbols that alias the same declaration, and the obligation has to reach both
names the entrypoint publishes it under.

Both do:

- `channelFor` — the re-exported name. The reachability rung resolves the
  helper's own declaration through the canonical symbol to this export name.
- `forwarded` — the caller. The call edge into `channel.js` is exact, so the
  call graph enumerates it as reaching.
- `Isolated` — the negative. It reaches nothing and must stay certified. A
  regression that answered "undecidable" for either of the other two would
  widen to `fallback-all` and mark `Isolated` too, which is what this export
  catches.

**The trap: `channel.js` must have no sibling `channel.d.ts`.** With one, the
specifier and the call both bind to the declaration file instead, the
implementation's symbol loses every reference outside its own module, and the
enumeration reports itself incomplete on purpose — the shape
`declaration-sibling-reach` pins. This fixture is the version of that shape
where identity *does* resolve, and it must keep the exact three-way answer.
