# Audited TanStack router 2.0.0-rc.9 renders route components (ADR 0334)

`rc8`'s claims and files, run against the published `@tanstack/solid-router@2.0.0-rc.9` (MIT, `sha512-iu9b8UeakVSqbltx1L0JDJKPxpe6dayqpUFjPn/GWj4DU31Ev7H+g1UakUvhp5phrGVcypxkmFsI8+C1x+AZPg==`, digest `f60f59d3…6f4b`; package READMEs omitted) and `rc8`'s `@tanstack/router-core@1.171.22`. Against rc.8 every route-option read is byte-identical; the changed files (observe attribution, hydration API, `useMatch`, compiled `Asset`/`link`/`CatchBoundary`) read no route option.
