# store-setter-draft-assign-shadowed

ADR 1067 negative: a local `createStore` passes `Math` to its updater. Its
setter has no dialect authority; the standard-member probe stays uncertifiable.
