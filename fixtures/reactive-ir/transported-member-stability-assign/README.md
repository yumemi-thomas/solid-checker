# Unknown Object.assign sources

Exact standard-library `Object.assign` with a nonliteral or spread source vetoes every transported object member
in the project: `FactoryConsumer` and `ContextConsumer` acquire no violation; `DirectBody` retains its ordinary
strict-read violation (this premise does not demote independent signal reads). `firstSlotTuple` and
`laterSlotSpread` cover tuple forwarding in written slot zero and `Object.assign({}, ...sources)`; the backend
retained/cold test isolates each layout and checks addition and removal. The reduced rc.13 primitive declarations
keep their published constraints.
