# created-accessors

**Claim (ADR 0690).** `children`, `mapArray` and `repeat` return a memo's accessor, and a call of a `createMemo` where it is made reads one. Each is a read like a memo's: a violation in a body, silent in a tracked scope, and open when its compute may suspend or the read is a member no row names.
