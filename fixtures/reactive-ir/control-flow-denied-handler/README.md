# control-flow-denied-handler

**Claim (ADR 0780).** A handler of an event the DOM delivers by itself (`onLoad`), written inside a control-flow callback, runs on whatever stack mounted its element: a read in it is open, as in a body. A handler of an event no host call delivers (`onClick`) keeps its proof.
