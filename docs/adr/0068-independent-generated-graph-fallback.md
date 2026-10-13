# Independent certification after a graph fallback

Kobalte Utils has a generated proposal containing multiple executable cases.
Its `scrollIntoViewport` claim remains unproved. Graph recovery fails on that
case, then the prior fallback retries the entire generated proposal and fails
again. The independent-case recovery path previously ran only when no graph
had been prepared.

The graph fallback now runs the same bounded independent selection on the
unaccepted generated proposal. Each trial is a native certification transaction;
the final selected union is projected as whole artifact cases and freshly
certified before publication. Claims inside a case are not removed or weakened.
The fallback records the actual published coordinates, exact native case
refusals, the combined graph refusal and all unpublished expected coordinates.
No graph-only case is assumed proved merely because it was not selected.

The existing-publication guard is captured before either native lane begins.
A refused attempt may itself create the output directory. That directory must
not be confused with a publication that existed before the attempt; conversely,
an existing directory still conservatively prevents reducing the proposal after
a refusal. This capture is shared by the plain and graph-fallback paths.

The focused test exercises the actual routing with injected native executors:
the graph creates a directory and refuses, independent trials retain exact
claims, the selected union is freshly published, and a previously existing
publication is preserved without attempting a smaller selection. Existing
independent-selection tests retain duplicate, empty-result, final-publication
failure and non-proof failure protections.

This changes orchestration and non-authoritative audit detail only. Contract,
receipt, proof and trust interfaces are unchanged. Real-package measurement and
full verification are pending for this slice.
