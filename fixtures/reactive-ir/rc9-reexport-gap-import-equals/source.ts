// A TypeScript import-equals of `solid-js`: `S` is the module's namespace
// object, exactly as `import * as S` binds it, and every use of it here is a
// member read. `S.untrack` names an export rc.9 declares and reaches nothing;
// `S.createErrorBoundary` names one of the five it does not.
import S = require("solid-js");

export const first = S.untrack(() => 1);
export const boundary = S.createErrorBoundary;
