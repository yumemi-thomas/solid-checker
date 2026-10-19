# rc9-reexport-gap-project-reexport

A project module re-exports two of the five names, one aliased (`export { createRevealOrder, $DEVCOMP as devComponent } from "solid-js"`), and `source.ts` imports from that module; the re-export names what it takes, so those are counted whether or not an importer uses them (`rc9-reexport-gap-none`). Expected: the `SC9014` notice carries the gap naming both, with one evidence step per name located in `boundary.ts`.
