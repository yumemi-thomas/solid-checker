# A router reached through a re-export (ADR 0324 refusal)

`router.ts` re-exports `createRouter` and another module calls it, so the call and its `history` are not where the premise can read them: not applied (`SC9012` stays). Router is the published `@solidjs/router@2.0.0-next.26` byte for byte (MIT; digest `e67af539…04d6`).
