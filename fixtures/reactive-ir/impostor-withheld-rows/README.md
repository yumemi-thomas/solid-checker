# impostor-withheld-rows

ADR 0874 (review R9): a call the impostor census withholds keeps the rows its standard run would place, each at its own site and
uncertifiable, beside the call's `standard-member-impostor` row.

- `scopeOf`: `target.slice(0, colon).trim()` keeps the `trim` row; an uncertain row deduplicates by exact span, not by start.
- `watch`: the effect's owner is not proven by the body's owned call alone; the withheld `forEach` callback may run it unowned.
- `App`'s `crumbs`: `db()` pending inside a withheld `flatMap` in a memo keeps SC5003's row, opaque, beside the unplaced one.

No row here is a violation.
