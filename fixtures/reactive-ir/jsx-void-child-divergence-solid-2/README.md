# JSX discarded-child coverage — Solid 2

This local integration fixture revalidates discarded child-content behavior at
published Solid RC.13 `5efaf260becb32293f2bcb4d32f8be72be6de674`.

Ordinary HTML void and
`<noscript>` child expressions are not checker-maintained transform
divergences at this pin. Template-root void child lists are explicit discarded
regions; nested void children remain live under Ryan's authoritative `next`
semantics and are tracked. A surviving source expression with no execution
entry remains an ordinary census gap.

`<menuitem>` children are classified from the producer trace and remain silent.
RC.13 rejects the removed `<keygen>` case as malformed HTML, so it cannot be
included in a fixture whose claim requires successful compiler facts. The
separate compiler-refusal check preserves that boundary. The module-scope
cleanup is the proven missing-owner control.
