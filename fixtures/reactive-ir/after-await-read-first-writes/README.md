# after-await-read-first-writes

**Claim (ADR 1000).** A write that reads its target first is a read of it. After an `await` of an async computation, a compound assignment
(`local.x += 1`), an update (`local.x++`) and a logical assignment (`local.x ||= 5`) on a store alias each Get the member before they Set it, and Chrome
rc.21 reports UNTRACKED_READ_AFTER_AWAIT there. The after-await member list (ADR 0976, S4) left every write target out, so each was silent.

- `Compound`, `Update`, `Logical`: an uncertifiable SC1002 (`read-after-await-unaccounted`) at the member, beside the store write's own row.
- `Plain`: a plain assignment reads nothing: no SC1002 row.
- `Local`: the same compound assignment on an object literal the function made: the row its plain read (`return local.x`) already has, since
  ADR 0976 proves no object literal plain. Chrome is quiet there (the legal twin `net/aw-compound-local-object` of the pattern lane).

Chrome: `fixtures/runtime-oracle/after-await-read-first-writes`.
