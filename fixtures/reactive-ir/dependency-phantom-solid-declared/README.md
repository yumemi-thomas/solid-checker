# dependency-phantom-solid-declared

**Claim (ADR 0981).** The negative of `dependency-phantom-solid`: the same package declaring its `solid-js` peer, selected by ADR 0297's default, and the read inside a tracked JSX expression, which subscribes: no SC1001 row.

Declaring the peer or not changes only which rule selects the package. The read stands in `<p>{location.search}</p>`, a tracked position, so nothing is reported on any build.
