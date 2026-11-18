# navigation-delivers-beforeunload-popstate

**Claim (ADR 1080, negative).** `location.assign("#next")` fires `popstate` before it returns (Chrome), and a `popstate` listener is here: the dispatch keeps withholding the handler's proofs.
