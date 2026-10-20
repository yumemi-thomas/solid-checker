# A namespace call is a call of the declaration it resolves to

A call through `ns.describe` counts as the call of `describe` only when its exact semantic entity resolves to the declaration of the call edge (`as typeof ns.describe` keeps identity), so `describe`'s caller-supplied-member obligation clears in the closed application; `escapes` read as a value, and modules also reachable as namespace objects used other than by static member (`Object.values`, `picking["picked"]`, `import("./loaded")`, barrels) keep `SC9012`; a local `ns` with a `describe` method confers nothing.
