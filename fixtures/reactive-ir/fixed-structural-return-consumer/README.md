# Fixed structural return consumer controls

ADR 0172 controls over stable tuple/object constructors with fixture-issued carrier contracts: bad tuple/object reads report, good object reads in JSX are clean; replacing/rebinding members, deleting, or passing a container to an unknown mutator drops the reactive identity; unknown indexes and computed dispatch stay uncertifiable (ADR 0282: an opaque `replace(result)` handoff makes tuple source calls SC9012). The stub preserves rc.9's branded `Refreshable<Accessor<T>>` first slot; `App.tsx` has no diagnostics against published rc.9.
