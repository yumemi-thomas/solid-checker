# Callback-result provenance consumer

Hand-stated authored contract; no generated acceptance catalog. Copy to
fixtures/reactive-ir/package-callback-result-consumer, add the two .gitignore
node_modules exceptions, and let coverage authorize a copied project using
.solid-checker/authorize-contract.json. reactive-package/package.json is
byte-identical to package-own-tracked-read-consumer, preserving the existing
manifest digest, artifact identity and resolved-import record shape.

PrimitiveNegative: fresh primitive results and getter-free literal objects
close only the described non-recursive result uses; transparent TS wrappers
around a producer literal are peeled. ProducerViolation: guaranteed direct
callback write, a proven violation. ExactReturnedArrowViolation: a guaranteed
result call in a created tracked owner, a proven write violation.
ExactReturnedArrowClean: the same exact body read remains tracked.

OpenResults: dynamic values, proxies, getter-bearing objects, spreads,
unknown census, external escape (including a closed tracked-use census with
an invocation-result return), named result aliases, ambient result calls
and result wrappers that lack an invocation/owner proof stay SC9012
uncertifiable. Getter code is not attributed to the factory callback context.
WrapperOpen stays uncertifiable; wrapper summaries cannot publish result-use
closure they did not prove. No snapshot was generated or updated.

These declarations belong to the synthetic package itself; they impose no
fake published-package signature. Every real-package probe is separately
checked against its existing installed published typings.

`noResultUses` exercises the explicit complete empty census. It adds no
result-dispatch obligation even for an unknown value. It does not erase the
ordinary producer callback's execution context or effects. Findings described
here are assertions for the lead's coverage run, not observed snapshots.
