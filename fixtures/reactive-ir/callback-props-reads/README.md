# Props and store reads in anonymous callbacks (ADR 0323)

A props or store read written in an anonymous function literal was dropped
before classification (`inside_unclassified_callback`), while a signal read in
the same literal reached `callee_callback_timing` and was reported. This
fixture pins the narrowed gate: such a read is classified exactly like the
signal twin where the callback reaches a **project callee**, an **inline
standard-library callback**, or an **IIFE**, at every layer up to the
component. Everywhere else it keeps the silence it had.

`App.tsx` (reported): project callee calling its parameter in its body (proven
violation for a component whose one caller passes a signal-backed prop;
uncertifiable for the exported one), an options-object property the callee calls
through a member (uncertifiable: ADR 0321 resolves the dispatch, not its
timing), a callee that hands the callback to a timer (clean: the use-site role
is a timer's), `forEach`/`map`, an IIFE, the store twins of each, and an IIFE
inside a project callee's callback.

`Silent.tsx` (refusal controls, no finding): a literal on a component
attribute, an unknown callee, `setTimeout`, `Promise.then`, a stored literal, an
IIFE or `forEach` inside a timer, the store twins, and an anonymous renderer
whose parameter is not a props object. The signal twin of each is an
uncertifiable `SC1001` today; closing that gap for props and store reads costs
about +800 uncertifiable on the 49-project corpus and is an owner decision.

Types: the published rc.13 declarations copied unchanged from
`standard-runtime-configuration`; the tree passes `tsc --noEmit`.
