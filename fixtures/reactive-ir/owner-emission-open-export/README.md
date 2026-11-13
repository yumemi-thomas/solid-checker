# owner-emission-open-export

**Claim (batch 11, R2 and R3 of the re-review of #221; ADR 0970).** The owner emission (SC4001) takes an exported component a tag renders for entered by that render alone only where every caller is visible. In a **closed** program the namespace census (ADR 0347) says whether the module's namespace is read by name or escapes with no name; in an **open** program (a package whose entry other code imports: `.solid-checker/runtime.json` `programBoundary: open`) the census holds nothing, and any export may be called by code the program does not hold. The component stop used to read the census only, so in an open program:

- **R2**: `Comp` is exported, a tag renders it, and the program's own timer enumerates the namespace (`Object.values(ns)`): certified, though the visible caller runs it with no owner.
- **R3**: `Exported` is exported, a tag renders it, and no file of the program calls it: certified, though an importer outside the program may (`import { Exported } from "pkg"; setTimeout(() => Exported())`). The same export with no tag was an obligation already, and a plain exported helper is one through `entered_by_unseen_code` (ADR 0934).

The component stop asks `entered_by_unseen_code` now: a function its module exports is entered by unseen code in an open program, and in a closed one where the namespace is read or escapes. The three read walks ask the same question.

| site | outcome | why |
|---|---|---|
| `onCleanup` in `Exported`, exported and rendered by a tag in an open program | SC4001 uncertifiable (`owner-callers-unenumerated`) | R3 (Chrome, oracle case `owner-emission-open-importer`: an external importer calls it, NO_OWNER_CLEANUP). **The false clean this fixes** |
| `onCleanup` in `Comp` of `lib.tsx`, exported, a tag renders it, `Namespace.tsx`'s timer enumerates the namespace | SC4001 uncertifiable | R2 (Chrome, oracle case `owner-emission-open-namespace`). **The false clean this fixes** |
| `onCleanup` in `Local`, not exported, rendered by a tag | silent | no importer reaches it: rendering is the only entry |
| `onCleanup` in `Inner` of `lib.tsx`, not exported | silent for SC4001 | the namespace does not reach it |

The SC3001 rows at the `onCleanup` of the exported components are the owner-call universe's (ADR 0969, policy A: an export of an open program is handed out) and stand on both builds. The closed-program control is `ClosedExported` in fixture `owner-emission-layer-stops`: exported, a tag renders it, no namespace read: silent.
