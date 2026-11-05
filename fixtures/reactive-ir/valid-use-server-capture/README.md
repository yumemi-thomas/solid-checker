# valid-use-server-capture

ADR 0706 (SSR audit D12). The directive pass hoists an extracted server function to module scope, so capturing a binding an
enclosing function declares is a compile error on both builds (`@solidjs/compiler@2.0.0-rc.21` `transformDirectives`: "server
functions cannot capture non-top-level variables"). Positives (`captures.ts`, one violation per function at its first capture):
an enclosing `const`, parameter, a default parameter, an arrow, an enclosing helper, the function's own name, a sibling server
function. Negatives (`compiles.ts`): module-scope `const` and `let`, the function's own locals, a type-only reference, a
module-level server function.
