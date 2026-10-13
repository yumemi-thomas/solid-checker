# Rows that have not reached certification

The completed callback-retry corpus has nine such rows. The
[inspection](2026-09-08-nonadvanced-frontier.json) binds each observation to
that report, its retained project, exact package identity and manifest digest.
All nine installations passed the report's integrity check; none timed out.
Raising execution budgets or retrying installation does not address these
observed failures.

- Four rows name unavailable published runtime targets: Kobalte Themes,
  Composites, and Animation's Solid 2 floor/head probes.
- Workers and Context refuse a missing declaration-closure module. Workers'
  retained `dist` contains only `index.js`, `index.d.ts`, `utils.js`, and
  `utils.d.ts`: the referenced `./types.js` has no declaration companion there.
  Context refers relatively to `../node_modules/solid-js/types/reactive/signal.js`.
  Replacing that path with another installation's declarations would change
  the artifact's resolution context, not authenticate the missing file.
- The Babel plugin and extension adapter have no generated runtime ESM export
  surface. This is not proof that their modules have no behavior, nor a reason
  to count an empty result as certified. CommonJS/module-effect support needs
  its own positive evidence.
- TanStack AI Solid UI has an explicit dependency-composition frontier through
  `@tanstack/ai-solid`; the latter already refuses in the graph path through
  its CommonJS dependency. No new root certificate is established here.

No package files, declarations, or coverage denominators were changed. This
inspection adds zero certificates and zero complete rows. It narrows the next
actions while the retained-preparation full corpus runs against frozen source
and binaries; it is not a substitute for that run's publication audit.
