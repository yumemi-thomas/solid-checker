# An object an install call was handed (ADR 0925)

After `Reflect.defineProperty(t, …)`, an alias, `.call` or a `this` install,
each access of `t`, a `const` of it or `this` is `installed-member`; a
handed-on operation marks all. `Clear.tsx`: none.
