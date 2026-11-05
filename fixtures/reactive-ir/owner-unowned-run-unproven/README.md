# owner-unowned-run-unproven

ADR 0714: a hook with an owned caller is an SC4001 violation only where an unowned call of it surely runs (`useModuleCall`, `useTimer`); a branch, `&&` or branching helper is an obligation; `useOwned` clean.
