# A stock history the project passes on (ADR 0324 refusal)

`router.ts` calls `memoryHistory()` directly, then hands it to `install`, which writes a `utils` onto it before `createRouter` reads `config.history.utils` (`dist/routers/factory.jsx:141,187`). A history used beyond `get`/`set` is not provably stock: the premise is not applied (`SC9012` stays). Router is the published `@solidjs/router@2.0.0-next.26` byte for byte (MIT; digest `e67af539…04d6`).
