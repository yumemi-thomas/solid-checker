# The route-definitions premise is withheld: modified-bytes (ADR 0335)

One byte of `dist/routing.js` differs from the published router, so its digest is not the audited one. The premise is not applied, so every route component in `Routes.tsx` (a copy of `route-config`'s; packages resolve from its `node_modules` by Node's lookup) stays undecided and its body read is uncertifiable, not a violation.
