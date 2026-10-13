# A tracked callback that may never run

Pins ADR 0244. A contract may state that a callback runs tracked, under an
owner the export creates, but possibly zero times or only when a resource is
read: `createLazyMemo`'s calc is the real case. Whenever such a callback runs,
its reads are tracked. So an accessor read written directly in it is not an
untracked read, even though the callback's execution is not guaranteed.

| component | export | verdict at `source()` in the callback |
| --- | --- | --- |
| `Misuse`, `NamespaceCorrect`, `OptionalWrite` | `lazyTracked` (closed, tracked, created owner) | clean |
| `Correct` | the same, behind `satisfies` | uncertifiable: the slot identity check does not see through `(…) satisfies T`, as in ADR 0183's helper |
| `MustRemainUncertifiable` | untracked, mixed and open slots | uncertifiable |
| `NestedEscape`, `UnsupportedFrames` | nested, async, generator and named frames | uncertifiable |
| `Shadowed` | no accepted export | uncertifiable |
| `ReturnedCallbackEscape` | returns argument 0 itself | uncertifiable |

`Misuse` keeps its proven `strict-read-untracked` violation at the caller's
top-level `value()` read. `OptionalWrite`'s write is not upgraded: the read
proof establishes no execution.

The contract is accepted out of band through `.solid-checker/authorize-contract.json`,
as in `package-own-tracked-read-consumer`. `tsc --noEmit` is clean on this
directory.
