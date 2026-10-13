# A semantic dependency can select another entrypoint of the same package

ADR 0012's forward entrypoint imports the package's own root. Its local runtime
closure contains `forward.js`; the dependency plan owns `index.js`. Both paths
are in one archive, which does not make the bare package import a relative
file edge. The native test plans the root first, replays the forwarder's actual
archive bytes with that dependency, and rejects an unplanned or forged target.

Catalog rebinding can preserve that already-bound edge without inventing a
local closure entry. Classification tests separately retain refusal without a
self edge and with a foreign/lookalike edge. No public contract snapshot is
added; the existing phase19 main-document count stays unchanged.
