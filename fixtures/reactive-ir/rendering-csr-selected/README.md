# rendering-csr-selected

An explicit client-only rendering selector (`.solid-checker/runtime.json` `"rendering": "csr"`) proves the server-rendering premise false, so a rule conditioned on "if this application server-renders" is silent rather than reporting an obligation the user discharged. Contrast `ssr-client-boundary` (premise proven: violation) and `ssr-client-boundary-csr`/`http-response-flush-csr` (unresolved: uncertifiable; absence of a server entry is not the discriminator). `LoudProfile` (SC5003's async arm, independent of the premise) is the positive control.
