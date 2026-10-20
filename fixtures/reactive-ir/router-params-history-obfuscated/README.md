# A history that supplies its own `utils` (ADR 0324 refusal)

`createRouter` hands `config.history.utils` to the router, whose `utils.paramsWrapper` replaces the params object (`dist/routers/factory.jsx:141`, `dist/index.js:1306`); here the key is assembled at run time and the history is an object literal, not a stock history, so the premise is not applied (`SC9012` stays). Router is the published `@solidjs/router@2.0.0-next.26` byte for byte (MIT; digest `e67af539…04d6`).
