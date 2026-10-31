# dispatch-read-through-closure

**Claim (ADR 0517).** A function's read summary carries a read made through `Function.prototype` (`r.call(null)`, `r.bind(null)()`) or a composite callee (`(slot ||= r)()`), as `local_access` judges them where they are written (ADR 0516). Before, the summary held only reads whose reference is the call's callee, so a closure called in the body that read this way was silent and the file certified.

`App.tsx`: `CallThroughClosure` and `BindThroughClosure` are violations (rc.17 in Chrome: `STRICT_READ_UNTRACKED`); `OrAssignThroughClosure` is uncertifiable (an undecided row); `CallInHandler` is clean.
