# router-creation-settled

**Claim (ADR 1066, twin of `router-creation`).** The same program; `mini-router`'s component calls the children function in `onSettled`, a
leaf callback: the `createMemo` written in the children arrow throws PRIMITIVE_IN_FORBIDDEN_SCOPE there (Chrome rc.21, oracle case
`fixtures/runtime-oracle/router-creation-settled`, with the router written in the case). Every creation stays uncertifiable. This is the
witness against reading a package call's result as `Known`: that shortcut cleaned line 21 here.
