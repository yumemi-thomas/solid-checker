# after-await-universe-none

**Claim (batch 11, slice 9B; ADR 0973).** The typed reason that ends a site written after an async function's own suspension with no row. Twin of `after-await-universe`, which holds the sites that keep SC1002's uncertifiable row. This program holds a tracked async computation (`Tracked`); there is no census reason (see `after-await-universe-census`).

| site | ends in | why |
|---|---|---|
| `stringify(1)` (in `if (false)`) and `stringify(2)` (after `return`) (`Dead`) | `after_await_accounting.account.never-runs` | `SiteReach` proves no execution runs the site (an arm a literal test rules out, code after an unconditional exit). |

The proof-log process test `after_await_accounting_process::the_reason_ends_exactly_its_sites` is the mutation test: `SOLID_CHECKER_IR_ABLATE_SILENCE=<id>` (or `all`) drops the reason, and the sites it ended must then each carry the row, with no other answer moved.
