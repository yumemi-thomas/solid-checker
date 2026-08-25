# JSX discarded-child coverage — Solid 2

This fixture revalidates discarded child-content behavior at
`dom-expressions#next`
`26e744fb4feb973a3652bfc45a8c3938ece667f0`.

Its source is byte-identical to the Solid 1.x sibling. Ordinary HTML void and
`<noscript>` child expressions are not checker-maintained transform
divergences at this pin. Template-root void child lists are explicit discarded
regions; nested void children remain live under Ryan's authoritative `next`
semantics and are tracked. A surviving source expression with no execution
entry remains an ordinary census gap.

Unlike Solid 1.x, Solid 2 has no parity-target-only void tags. `<keygen>` and
`<menuitem>` children are therefore certified from the producer trace and
remain silent. The module-scope cleanup is the proven missing-owner control.
