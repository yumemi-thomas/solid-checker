# A declaration does not stand for its runtime module

In a closed application (ADR 0219) `helper` is entered by a direct call and a type query, which alone would clear its obligation; but a declaration file is not a project source, so the obligation stays (`SC9012`) wherever a runtime module may re-export it: `./bridge.js` resolves to `bridge.d.ts` but loads `bridge.js`; directory `./bridge2` has `types` and `main` pointing at different files; installed `linked` resolves to a declaration while its source and runtime `main` are in the program.
