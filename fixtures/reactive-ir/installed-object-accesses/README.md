# Each access of an installed object (ADR 0643 amendment)

A store put on an object as its prototype makes each Get and write of that object
the store's. The install's own obligation does not place the access that raises,
so each access this file writes of the object (the literal, its binding, a call
handed it whole, placed at that call) is uncertifiable (`installed-member`). Where
the `__proto__` key is handed on, an object written through a non-literal key
counts as installed. A plain prototype installs nothing.
