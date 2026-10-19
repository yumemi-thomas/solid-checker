# A consumer of a registered computation

ADR 0114/0161: an accepted contract stating a `compute` in `computations` tells a consumer the export registers a computation on its caller's owner, so an unowned call is `SC4001`: `min: 1` (`startTickerAlways`) is a violation, `min: 0` (`startTicker`) uncertifiable, `startTickerSilent` (no `computations`, `creates` closed) clean; inside `Ticker` all are clean. `callbacks`, `reads`, `creates` and `returns` are closed. Ships `.solid-checker/authorize-contract.json`; manifest is `package-plain-return-consumer`'s; signatures are `() => void`.
