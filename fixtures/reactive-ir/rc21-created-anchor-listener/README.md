# A click listener on the path keeps the listener veto

ADR 0640, the guard. The same anchor bubbles to a `click` listener on `window`, so the dispatch delivers to project code and the
listener proofs stay withheld (ADR 0642). ADR 1099: the path holds no JSX element, so the JSX handlers' owner and read proofs no
longer wait on it (the counter handler's SC2001 and SC1001 rows are gone).
