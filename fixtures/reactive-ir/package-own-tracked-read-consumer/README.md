# A package's own tracked read

Pins ADR 0239. A contract may state a read of the export's own reactive state.
Where that read runs untracked on the call's stack, Solid warns at every use,
and the checker reports it as the package's own read (`SC1001`,
uncertifiable). Where the contract states it `tracked`, under an owner the
export creates, the read runs inside the export's own computation. It is
then not a read in the caller's tracking context, so nothing is reported.

| component | export | read | verdict |
| --- | --- | --- | --- |
| `Watched` | `watchStatus` | tracked, created owner | clean |
| `Peeked` | `peekStatus` | untracked, at the call | SC1001 uncertifiable |

The contract is accepted out of band through `.solid-checker/authorize-contract.json`,
as in `package-merged-props-consumer`. `tsc --noEmit` is clean on this
directory.
