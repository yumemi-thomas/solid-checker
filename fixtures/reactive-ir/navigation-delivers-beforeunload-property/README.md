# navigation-delivers-beforeunload-property

**Claim (ADR 1080, negative).** `window.onbeforeunload = …` registers a listener by assignment: `history.go` keeps withholding the handler's proofs.
