# handed-root-patch-definer-key-other

ADR 0951: `Number` handed to a helper that defines `isNaN` under the literal key `"isNaN"` leaves `isFinite` the declared
member: a reflective definer put to a literal key defines that key and no other, so the call is no longer withheld
(before: SC9012 `standard-member-impostor`, the blanket spelling `ReflectiveDefinitionSpelling`). The function the program
hands `Number.isFinite` never runs; Chrome reports nothing at the read of `r1` in `sf1`.
