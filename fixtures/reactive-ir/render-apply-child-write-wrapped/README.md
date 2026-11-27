# SC2001: a render-effect write in a wrapped child (ADR 1227)

The child is rendered in another component's children, which no fact places on the creation pass: uncertifiable. rc.21 throws when `Wrap` renders them, so this is a recall miss. Chrome: `fixtures/runtime-oracle/render-apply-named-write`.
