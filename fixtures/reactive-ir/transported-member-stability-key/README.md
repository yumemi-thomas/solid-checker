# Transported member stability: keys the literal fact cannot name

ADR 0273 vetoes a transported member when a visible write names it. An exact
`Object.defineProperty`, `Reflect.set`, `Reflect.defineProperty` or
`Reflect.deleteProperty` call whose key the argument-literal fact cannot name
(here a template literal, an identifier, and a tuple spread that shifts every
argument position) could write any member, so it vetoes
every transported member in the project. FactoryConsumer and ContextConsumer
therefore report no violation; DirectBody's direct signal read, which does not
depend on member stability, stays a violation.
