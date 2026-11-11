# leaf-visited-entry

**Claim (ADR 0961).** The leaf-owner walk of a call keeps the functions it has entered; a function entered a second time under another context is walked again (the key is the function and the entry: its kind, whether it is guaranteed, the argument standings the call proves, the class `this`). Before, `make(true); make(false)` skipped the second call and `isPending(g); g()` took the direct call for the one `isPending` already walked, whose flush it catches: Chrome throws at the second entry, and the checker was silent.

`App.tsx`: `TwiceForbidden`, `SwallowedThenDirect` (violations at the helper's call), `TwiceInCallback`, `SwallowedThenDirectInCallback` (the entries written in a closure the host's callback declares and calls: violations); `TwiceUnknown` (`make(x); make(!x)`, an argument the walk cannot decide: an uncertifiable row, never clean). `TwiceSkipped`, `SwallowedOnly`, `Recursion` (entered only the tolerant way, or recursing: clean).
