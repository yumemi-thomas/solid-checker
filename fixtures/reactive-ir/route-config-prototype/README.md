# The route-definitions premise is withheld: prototype (ADR 0335)

`Read.tsx` installs `transformUrl` on `Object.prototype`; the router calls `config.transformUrl(...)` with the config as `this`, which reaches its `routes`. Any prototype spelling withholds the premise, so every route component in `Routes.tsx` (the sources and packages of `route-config`) stays undecided and its body read is uncertifiable, not a violation.
