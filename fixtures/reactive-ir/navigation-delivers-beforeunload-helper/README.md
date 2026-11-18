# navigation-delivers-beforeunload-helper

**Claim (ADR 1080).** A listener helper's type parameter is followed to its calls, as `@solidjs/router`'s `bindEvent(target, type, handler)`: every call passes a literal of another event (`scroll`, `pagehide`), so nothing listens for `beforeunload` and `history.go` withholds no proof (the handler's SC2001 and SC1001 certified). A `typeof addEventListener` guard (an inert operand) registers nothing, and its `addEventListener("online", …)` names another event.
