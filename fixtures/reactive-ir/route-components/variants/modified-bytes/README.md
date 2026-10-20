# The route-components premise is withheld: modified-bytes (ADR 0334)

One byte of `dist/source/Match.jsx` differs from the published router, so its digest is not the audited one. The premise is not applied, so every route component in `Routes.tsx` (a copy of `route-components`'s; packages resolve from its `node_modules` by Node's lookup) stays undecided and its body read is uncertifiable, not a violation.
