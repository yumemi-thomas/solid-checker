# component-census-no-entry

**Claim (ADR 0403).** A census that accounts for every reference proves a component only when something renders it. A function whose references are its declaration alone is dead, and a lowercase or unexported one is no host's component; module evaluation has no owner. `Unrendered.tsx`: `SC2001` is uncertifiable, not a violation, for `Dead`, `dead`, `renderRow`, `makeRow`, a module-level `.map` callback and IIFE returning JSX, and `Row` (the tag in `Shadows` names a local). `Rendered.tsx`: `Tagged` (a tag) and `Page` (exported, capitalized: module surface) stay violations, as `Route` does in `DefaultPage.tsx` (a default export).

Stub: rc.17 `solid-js` package manifest; declarations abbreviated.
