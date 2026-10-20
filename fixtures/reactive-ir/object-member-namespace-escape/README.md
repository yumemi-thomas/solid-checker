# ADR 0299 review: namespace-base

On the default source route exported receivers are handed to opaque code through computed string namespace accesses and an `export *` barrel namespace, so the original bodies may be replaced at runtime: neither the read at `App` nor the original cleanup is a proven violation, the exported cleanup's definition-site obligation stays, and `Callback` keeps an obligation. Solid signatures copy `object-member-import`'s published rc.13 declarations; `opaque-mutate` carries its full runtime.
