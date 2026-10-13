# closed-export-member-dispatch

**Claim (ADR 0203).** In a closed program (a private `package.json`, ADR 0193), an exported helper that invokes a member of its own parameter raises no obligation at its declaration when every way of entering it is a call expression. Each of those calls selects the member's implementation from its own argument, or keeps its own obligation, as a call of an unexported helper does.

| Case | Closed (`closed-export-member-dispatch`) | Open (`-open`) |
| --- | --- | --- |
| `describe`, entered only through calls | no declaration obligation | `SC9012` at `helpers.tsx:5` |
| `escapes`, also handed out as a value (`kept`) | `SC9012` at `helpers.tsx:10` | `SC9012` |
| `Card`, entered through JSX | `SC9012` at `helpers.tsx:17` | `SC9012` |
| `describe({ label() { … } })` | clean: the method is what runs | clean |
| `describe(props.item)` | `SC9012` at the call site (`App.tsx:8`) | same |

The other findings (`SC1001` on `props.item`, and on `props.format.label` in the open twin) belong to the props rules and are not part of this claim.

The two fixtures share `App.tsx` and `helpers.tsx`; only `package.json` differs. The stubs are copied from `feedback-tiers`.
