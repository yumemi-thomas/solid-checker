# server-function-rich-args-entry

ADR 0707 (SSR audit D10). The rich-args opt-in is the call, not the import: `enableRichArguments()` installs the codec. Proven
here: the client entry (`main.tsx`) calls it unconditionally at module scope before its `render`, so the `Date` and `Set`
arguments in `App.tsx` are encoded and SC7007 is silent. `server-function-rich-args-enabled` (a module-scope call in a component
file, no render) is the uncertifiable twin.
