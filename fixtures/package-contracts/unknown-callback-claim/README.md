# Unknown callback claim

The runtime artifact forwards an exported callback parameter to a declaration
whose execution timing is unavailable. Package-contract generation must emit a
partial export with `callbacks: { "status": "unknown" }`, retain the known
`plain` sibling, and put one grouped unknown-claim item in the review plan.
