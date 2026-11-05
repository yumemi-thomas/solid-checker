# SC5007: `<Loading on>` handed a function

rc.21 warns `LOADING_ON_ACCESSOR` on the boundary's first run when `on` evaluates
to a function, and `on` is typed `any`, so `tsc` is silent. A function literal and
a value Type Facts call a function are proven (ADR 0624). A union that may be one
is unproven, and a value, an array or an `unknown` state nothing.
