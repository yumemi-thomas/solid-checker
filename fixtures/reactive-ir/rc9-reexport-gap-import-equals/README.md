# rc9-reexport-gap-import-equals

`import S = require("solid-js")` whose every use is a member read binds the namespace object as `import * as S` does: `S.untrack` reaches nothing, `S.createErrorBoundary` names one of the five rc.9 re-exports without declarations (`rc9-reexport-gap-none`); a passed or stored `S` keeps the gap open (`AstFacts::import_equals`, schema 46). Expected: the `SC9014` notice carries the gap naming only `createErrorBoundary`. `tsconfig.json` uses `"module": "Preserve"`.
