# ADR 0299 review: tdz

`App` calls a closure before its function-local receiver initializes, catches the ReferenceError, then initializes the object without calling it, so its cleanup never registers and no violation is proven; shared cases, the import cycle and `module-tdz` share the boundary, captured receivers are refused, `Initialized` keeps its proven cleanup violation, and `CallbackTdz` stays uncertifiable. Solid signatures copy `object-member-import`'s published rc.13 declarations.
