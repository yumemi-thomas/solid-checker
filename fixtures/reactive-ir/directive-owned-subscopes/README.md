# directive-owned-subscopes

SC6001 requires an intrinsic ref apply phase with no owner. rc.13 runtime
probes distinguish eager arguments/IIFEs (component setup), direct and array
apply callbacks, default initializers, roots, timers and opaque helpers.
Direct apply/timer allocations violate; unknown helper/returned/nested/generator
execution remains uncertifiable. `runWithOwner` may receive null. Explicit roots
are lifetime obligations (ADR 0355): they may be empty or disposed. RootLifetime
pins namespace, factory and shadowed cases. Signatures copy signals rc.13
`dist/types`.
