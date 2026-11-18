# navigation-delivers-beforeunload-listener

**Claim (ADR 1080, negative).** `window.addEventListener("beforeunload", …)`: a listener may run on the navigation's stack, so `history.go` keeps withholding the handler's proofs (SC2001 and SC1001 stay `function-run-unplaced`).
