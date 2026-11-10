# impostor-withheld-rows

ADR 0874: a withheld call keeps the rows its standard run would place, each uncertain at its own site, beside its
`standard-member-impostor` row. `scopeOf`: `trim` keeps its row (dedup by exact span). `watch`: the withheld `forEach` callback
may run the effect unowned. `crumbs`: `db()` pending in a withheld `flatMap` keeps SC5003's row. No violation.
