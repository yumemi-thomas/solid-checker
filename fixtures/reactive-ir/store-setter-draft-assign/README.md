# store-setter-draft-assign

ADR 1067: an inline updater of the exact, immutable second binding returned by
rc.21 `createStore` receives the store's own proxy. `Object.assign` on that
parameter reaches no standard root. The `Number.isFinite` probe certifies once
this is the program's last open write. The iteration premise must hold too.
