# navigation-delivers-beforeunload-helper-open

**Claim (ADR 1080, negative).** One call of the listener helper passes a type nothing names (`String(Date.now())`), so its parameter may be `beforeunload`: `history.go` keeps withholding the handler's proofs.
