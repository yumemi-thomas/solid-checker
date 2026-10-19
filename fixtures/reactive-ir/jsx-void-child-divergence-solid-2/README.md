# JSX discarded-child coverage (Solid 2)

Revalidates discarded child-content behavior at published Solid RC.13 `5efaf260becb32293f2bcb4d32f8be72be6de674`: HTML void and `<noscript>` child expressions are not checker-maintained transform divergences here; template-root void child lists are explicit discarded regions, nested void children stay live and tracked, a source expression with no execution entry is a census gap; `<menuitem>` children come from the producer trace; the removed `<keygen>` is malformed HTML under RC.13 (a separate compiler-refusal check keeps that boundary).
