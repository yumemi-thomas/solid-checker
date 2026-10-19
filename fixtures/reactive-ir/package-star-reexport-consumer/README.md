# Accepted open claims through star barrels

ADR 0301: `star.ts` re-exports an accepted package whose `isReadyOpen` leaves `returns` open; its demanded binding, a second star hop and an exact `export * as ns` member keep the contract and raise `SC9005` (as the named form does); `isReady` (closed plain return) clean; `shadow.ts`'s local `isReadyOpen` shadows the star (clean). Contract and manifest bytes are those of `package-plain-return-consumer` with the authorization importer on `star.ts`; the package is synthetic with no Solid primitive typings.
