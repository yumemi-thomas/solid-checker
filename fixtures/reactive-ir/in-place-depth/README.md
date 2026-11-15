# The in-place climb's bound is a seventeenth layer

ADR 0988. `climb_in_place_layers` crosses at most sixteen function layers (IIFEs, a helper's callback, a named closure's one call) and says why it stopped
(`InPlaceStop`). The bound is a *seventeenth* layer; sixteen layers that end at a function that is no layer (a memo's callback) or at module scope are
placed. Before, the sixteenth crossing set the bound whether or not anything sat above it.

`Memo15`, `Memo16`, `Memo17`: `resolve` in that many nested IIFEs in a memo (SC2004). 15 and 16 are violations (Chrome rc.21 throws at every depth), 17 is
the bound and stays an obligation. `act15`, `act16`, `act17`: `flush()` in an action step the same way (SC2006).
(The module-scope end of the climb, `NoFunction`, is a unit test: a chain of IIFEs at module scope is an obligation at any depth, `function-run-unplaced`, before and after.) The program is open (no closed-boundary assertion), the case the bound matters for: nothing else places a site 17 layers deep.
