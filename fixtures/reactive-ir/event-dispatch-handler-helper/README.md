# event-dispatch-handler-helper

Claim (ADR 0437): every site of the program is removed together, by one induction over time. A dispatch in a plain function that only an event handler calls runs on the stack of that handler's delivery, as a dispatch written in a handler does, so neither site withholds the event proofs: the third handler's effect creation is a violation (SC4001); its write and read are clean. A monotone iteration from the all-sites state could not have removed either, since no proof exists while a site remains.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
