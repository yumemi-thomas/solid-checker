# export-star-as-solid

**Claim (ADR 0780).** `export * as Solid from "solid-js"` names every primitive, with no `import * as` of Solid in the project: a read of a source made through it is a violation. The same re-export of a project module names nothing.
