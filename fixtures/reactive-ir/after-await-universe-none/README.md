# after-await-universe-none

**Claim (batch 11, slice 9B; ADR 0973).** The typed reasons that end a site written after an async function's own suspension with no row. Twin of `after-await-universe`, which holds the sites that keep SC1002's uncertifiable row. This program holds a tracked async computation (`Tracked`); there is no census reason (see `after-await-universe-census`).

| site | ends in | why |
|---|---|---|
| `double(2)`, `quad(2)`, `helpers.triple(2)` (a namespace import of another module) and `calc.halve(4)` (a method of a `const` object literal: an exact member, ADR 0279) (`Pure`) | `after_await_accounting.account.callee-reads-nothing` | the callee is a project function the lookup resolves exactly, and its whole transitive closure (`quad` calls `double` twice, each call resolved exactly to a project function again) has no read of the universe, no generator, no JSX, no tagged template, no construct and no primitive: whatever the call runs reads no reactive source. A run no call shows (a getter, a coercion, an iterator, a Proxy trap) is the member-route net's (ADR 0963): the read written in such code has a row of its own |
| `stringify(1)` (in `if (false)`) and `stringify(2)` (after `return`) (`Dead`) | `after_await_accounting.account.never-runs` | `SiteReach` proves no execution runs the site (an arm a literal test rules out, code after an unconditional exit). The callee calls the standard library, so the callee reason does not end these two |

The proof-log process test `after_await_accounting_process::each_reason_ends_exactly_its_sites` is the mutation test: `SOLID_CHECKER_IR_ABLATE_SILENCE=<id>` (or `all`) drops a reason, and the sites it ended must then each carry the row, with no other answer moved.
