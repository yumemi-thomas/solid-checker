# after-await-universe-none

**Claim (batch 11, slice 9B; ADR 0973).** The one typed reason that ends a site written after an async function's own suspension with no row, and what it does not end. Twin of `after-await-universe`, which holds the sites that keep SC1002's uncertifiable row. This program holds a tracked async computation (`Tracked`); there is no census reason (see `after-await-universe-census`) and no callee reason.

| site | outcome | why |
|---|---|---|
| `stringify(1)` (in `if (false)`) and `stringify(2)` (after `return`) (`Dead`) | no row: `after_await_accounting.account.never-runs` | `SiteReach` proves no execution runs the site (an arm a literal test rules out, code after an unconditional exit) |
| `double(2)`, `quad(2)`, `helpers.triple(2)` (a namespace import of another module) and `calc.halve(4)` (a method of a `const` object literal: an exact member, ADR 0279) (`Pure`) | SC1002 uncertifiable, `read-after-await-unaccounted` | **no callee reason.** The callee is a project function the lookup resolves exactly, and its whole closure resolves exactly to project functions that read nothing the checker lists. The first build ended these calls; the review of #233 found that a closure is a list of reads and calls, and that a read reaches a continuation through values no call shows (a store read through an alias, a stored generator, a trap or a getter installed by reference, a props alias: the programs of `after-await-universe`'s `ByClosure`). A sound reason needs facts the checker does not have (which values are plain data); the call keeps its row |

The proof-log process test `after_await_accounting_process::the_one_reason_ends_exactly_its_sites_and_no_callee_reason_ends_a_call` is the mutation test: `SOLID_CHECKER_IR_ABLATE_SILENCE=<id>` drops the reason, and the sites it ended must then each carry the row, with no other answer moved; the four calls of `Pure` are the only rows of the program.
