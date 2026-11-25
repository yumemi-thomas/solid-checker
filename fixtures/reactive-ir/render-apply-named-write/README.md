# SC2001: a named render-effect apply (ADR 1227)

ADR 1227: the apply is a `const` handed by name, its one use. A render effect's first apply runs at its call under the component's owner, so the write throws REACTIVE_WRITE_IN_OWNED_SCOPE on rc.21. A violation. 
Chrome: `fixtures/runtime-oracle/render-apply-named-write`.
