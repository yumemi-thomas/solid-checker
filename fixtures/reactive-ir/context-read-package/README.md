# context-read-package

**Claim (ADR 0352, ADR 0290).** A package hook that calls `useContext` is followed through the package's source, selected by `.solid-checker/runtime.json`, and reported at the project call. `HandlerHook` (an event handler) is a violation; `BodyHook` (a component body) is silent; `ConditionalHook` (runs only when `armed`) is uncertifiable; `UnselectedHook` (`dep-unselected`, installed but not selected) is SC9005.

Source: `node_modules/dep-dialog-context/dist/index.jsx` lines 2 to 9 are `@kobalte/core` 2.0.0-alpha.0 `dist/dialog/DZMZsnbW.js` lines 15 to 22, verbatim (sha256 `6952d9a33af725c2a72ab280ab75473a15ceb167b38500229ec3a18a7aaceb59`; kobalte `sha512-6l5wJRkk/CRXWzbzwYeNfv38XHWKoL3Bj9ykJQexGcmyM2TQV4IpR1E/wDbhygT9TsI3LYP5qgdLj/l2xx+qiw==` from its lockfile), plus the import and export. The manifest is this fixture's own. `index.d.ts` returns `unknown`; no case reads the value. tsc is clean with the two modules declared ambient.
