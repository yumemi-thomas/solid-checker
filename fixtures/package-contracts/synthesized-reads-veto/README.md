# `synthesized-reads-veto`

The subject of [ADR 0163](../../../docs/adr/0163-a-synthesized-veto-observes-reads-through-a-tracking-memo.md)'s
synthesized `reads: []` veto, and the proof that the veto can fail.

Not in `corpus.json`: nothing here is generated. `synthesized_vetoes_tests.rs`
writes the veto module the checker synthesizes for each export, places this
package and the **real** `@solidjs/signals` of every audited release (the
`SOLID_CHECKER_RC{3,6,9}_ARCHIVE_ROOT` installs `make test-rust` provisions)
under one `node_modules`, the way the private probe workspace does, and runs
the module. The package has no `node_modules` of its own for that reason: a
stub would be exactly the loosened copy the veto must never be tested against.

| export | reads at the call? | the veto must |
| --- | --- | --- |
| `readsNothing` | no | stay quiet |
| `readsOwnSignal` | a module-level signal it owns | contradict |
| `readsCreatedSignal` | a signal the call creates | contradict |
| `readsCreatedMemo` | a memo the call creates | contradict |
| `readsThenThrows` | the owned signal, then throws | contradict (and record `sample-threw`) |
| `throwsWithoutReading` | no, and never completes | throw: incomplete, never a pass |
| `readsUntracked` | an `untrack`ed read | stay quiet — a stated limitation |
| `readsLater` | a read in a microtask | stay quiet — a stated limitation |
| `createsAReadingMemo` | a created memo reads, the call does not | stay quiet — that memo's read, not the call's |

The last three are pinned so the limitations the synthesized entry states
(`sampling_limitations`) are behaviour, not prose: if the veto ever starts
seeing one, the row moves and the statement has to move with it.

`index.d.ts` declares every parameter `number`, so the sample is fully
described by the value facts; that is the only kind of signature the veto is
synthesized for.
