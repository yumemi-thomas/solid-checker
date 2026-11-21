# Context provider children refusals (ADR 1123: F2, F3 and N1 of the withdrawn arm)

ADR 1123 withdrew the `context-provider-children` arm, which read a dialect provider's written children in place (a child element or expression runs where the provider element is created). rc.21's provider returns `children(() => props.children)`, a memo its reader runs, so the children run where the provider's value is read, and no test of where the provider *element* is written says where that is (review round 2, N1; round 1's F2 and F3 were the cases the first tests missed).

Every case keeps its SC3001 and SC6002 rows (uncertifiable), as any component's children do, `ReturnedApp` and `NestedApp` included (the withdrawn arm certified those two). The cases are the redo's acceptance set (`docs/open-work.md`): a transitive placement may certify `ReturnedApp` and `NestedApp` only (a provider whose value is consumed where it is created); each other case must keep its rows.

- F2, a tag that is no provider: `WrittenApp` (a written `let`), `PatternApp` (a pattern's name, here an unread component), `ImportedApp` (another module's context).
- F3, the provider element's value goes elsewhere: `StoredApp`, `PassedApp`, `HelpedApp`, `HandedApp`.
- N1, the provider is the whole return of a component and that component's element goes elsewhere: `ReturnedStoredApp`, `ArrayHandedApp`, `NestedStoredApp`.

Stub: `context-provider-proof`'s. Chrome: runtime-oracle `context-provider-children-handed`, `-package` and `-placed`.

**ADR 1147 (arm 1, `context-provider-children-owned`).** Every child here creates a memo, an owner code (SC3001's PRIMITIVE_IN_FORBIDDEN_SCOPE, SC6002's
PRIMITIVE_IN_EFFECT_CALLBACK). rc.21 runs a provider's children inside its `children()` memo, whose owner is the memo wherever the memo is read, so the owner
codes do not depend on where the provider's value goes: arm 1 ends them at every child of an exact provider (the F3 and N1 cases too; Chrome:
`context-provider-children-owned`), and the F2 cases keep them (no exact provider). This fixture stays the stack acceptance set (arm 2: a flush, SC2006,
SC2008, SC1002, where only `ReturnedApp` and `NestedApp` may certify); the stack pins are `context-provider-children-owned`'s `FlushStoredApp` and
`FlushInnerApp` and the Chrome case `context-provider-children-placed`.
