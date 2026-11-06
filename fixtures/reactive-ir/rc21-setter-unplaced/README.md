# rc21-setter-unplaced

ADR 0721: a setter or `refresh` call in a function no edge places (getter helper, method, `Map` slot, constructor, field) is an SC2001 obligation, blocker `function-run-unplaced`; Chrome throws. `Direct` and the module tag are violations; module initialization, `ownedWrite` and an unnamed class are silent.
