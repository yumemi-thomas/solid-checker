# program-boundary-closed

`.solid-checker/runtime.json` asserts `programBoundary: "closed"`: the analyzed
files are the whole program, so an export reaches no caller outside them.

This is evidence the analyzer **cannot derive**. Nothing inside a tsconfig
proves that nothing outside it imports from the tsconfig, which is why an
exported component's prop backing and an exported helper's owner stay proof
obligations however completely the project itself is analyzed. It is the same
class of user-supplied premise as `rendering`, and it is the single largest
source of obligations in the corpus.

It removes exactly one assumption: that an **additional, unseen** caller
exists. It licenses no guessing. Every caller must still be enumerated, every
reference must still resolve to a use the analyzer understands, and a missing
reference list is still the absence of a fact rather than proof of no callers.

Same files, both boundaries:

| Case | Open (default) | Closed |
| --- | --- | --- |
| `StaticOnly` — exported, every visible caller static | uncertifiable | **silent** |
| `Dynamic` — exported, a visible caller passes a signal | violation | violation |
| `setupRooted` — exported helper, sole call site inside `createRoot` | uncertifiable | **silent** |
| `setupBare` — exported helper, called at module scope | violation | violation |
| `PassedAsValue` — exported component handed to a receiver as a value | uncertifiable | uncertifiable |

Three rows are the point. `Dynamic` and `setupBare` prove the assertion does
not *create* findings: a witness and an unowned call site are violations either
way, and closing the program makes `setupBare` no less provable. `PassedAsValue`
proves the assertion is not a blanket amnesty — closing the program says
nothing about what a receiver does with a component handed to it, so that one
keeps its obligation.

The paired open-world fixtures keep the default and keep their obligations:
`engine/eslint-reactivity-*` and `eslint-plugin-corpus*` exist to pin what is
provable *without* this premise, and must never gain a selector.
