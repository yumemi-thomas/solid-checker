# host-callback-read-timing

**Claim.** A read in a callback a host API retains and may invoke on its invoker's stack, written in a component body, is `SC1001` **uncertifiable** (pending reads `SC5001`), never a proven violation (`execution_role::host_callback_timing`): `addEventListener` argument 1, `Function.prototype.bind` arguments, `PromiseLike.then` callbacks, Geolocation callbacks. A project object's own `addEventListener` calling the listener at once is a violation; `setTimeout` is silent. Variants: bound dispatch uncertifiable; host-only and homonyms owner violations (ADR 0356). Stub: `fresh-stack-scheduler-read-role`'s (published rc.3).

The components are exported: a function nothing renders is no proven component body (ADR 0403).
