# Resolution closure outranks a bounded syntax seed

`index.js` deliberately imports more than 150 names from `./big.js`. The shape
historically exceeded a bounded syntax seed even though the authoritative
module graph opened `big.js` and contract semantics depended on it.

Temporary-v2 artifact acquisition binds the exact resolved closure, so the
artifact case digest includes both files regardless of seed convenience. The
proposal keeps its six unresolved claim leaves and four closure candidates
local to the export; it does not claim that a missing seed proves a missing
module. `expected.json` and `expected-proposal.json` pin the closure and semantic
identity.
