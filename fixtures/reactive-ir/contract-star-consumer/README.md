# Accepted open claims through star barrels

ADR 0301: `star.ts` re-exports an accepted package whose `isReadyOpen`
leaves `returns` open. Its demanded binding, a second star hop, and an exact
member of `export * as ns` retain the same contract and raise SC9005
(uncertifiable), as the named form in `package-plain-return-consumer` does.
`isReady` has a closed plain return and stays clean. `shadow.ts` exports a
local `isReadyOpen`; that local binding shadows the star and stays clean.

The package is synthetic and its declarations describe its boolean returns;
there is no published package stub. All contract and manifest bytes are those
of `package-plain-return-consumer`, with the authorization importer moved to
`star.ts`. This fixture contains no Solid primitive typings. The process
regressions additionally pin ambiguous stars (TS2308, with no checker claim),
identical bindings, unknown competitors, and direct package namespace exports.
