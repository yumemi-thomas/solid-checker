# JSX discarded-child coverage — Solid 2

This fixture revalidates discarded child-content behavior at
`dom-expressions#next`
`c7e83a1bb0fc8e8f7fad37a7523db9fcce568820`.

Its source is byte-identical to the Solid 1.x sibling. Ordinary HTML void and
`<noscript>` child expressions are not checker-maintained transform
divergences at this pin. A surviving source expression with no execution entry
is an ordinary census gap and can only produce an uncertifiable SC1001; an
explicit discarded-region fact suppresses the deleted operation entirely.

Unlike Solid 1.x, Solid 2 has no parity-target-only void tags. `<keygen>` and
`<menuitem>` children are therefore certified from the producer trace and
remain silent. The module-scope cleanup is the proven missing-owner control.
