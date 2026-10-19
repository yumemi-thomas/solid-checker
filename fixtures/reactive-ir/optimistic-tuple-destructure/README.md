# Destructuring 2.0's optimistic tuples binds a reactive source

`Dialect::returns_reactive_tuple` (ADR 0111) must name `createOptimistic` and `createOptimisticStore`: the declaration form (`App.tsx`) is the control, the assignment form (`Assign.tsx`) loses `count` and `row.label` if they are dropped; each function reads in the compute (silent) and the apply (the SC1001). The stub keeps the two-slot tuple shape and first-slot callability of the published returns; `value: Exclude<T, Function>` is byte-faithful; both sources pass `tsc-oracle check --dialect v2` with 0 diagnostics.
