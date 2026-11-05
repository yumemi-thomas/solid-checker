# rc21-create-in-effect-unplaced

Claim (ADR 0710): `createMemo` in a function no call edge places (module-level object method, class method, getter) that an apply reaches runs with no owner, and rc.21 dev throws `PRIMITIVE_IN_EFFECT_CALLBACK` (Chrome): an SC6002 obligation, blocker `function-run-unplaced`. `createRoot` in a member is one too; inside it or `runWithOwner`, silent. Kept: a creation in the apply is a violation.
