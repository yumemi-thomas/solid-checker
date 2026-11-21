# SC9012: a replaced standard member keeps a whole-store form open (ADR 1178)

The twin of `store-whole-standard-census`: the program writes `Object.values`, so `Object.values(row)` is withheld (ADR 0840,
`standard-member-impostor` at the call) and ADR 1178 does not end the walk at it; the write also opens ADR 0803's premise for
every store tuple (`tuple-iteration-written` at both references). Every reference stays uncertifiable, as with the ablation
`SOLID_CHECKER_IR_ABLATE_SILENCE=value_use.whole_store_argument` (identical findings).
