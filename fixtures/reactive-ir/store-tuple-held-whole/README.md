# SC1001: a store tuple held whole (ADR 1138)

`const st = createStore(…)` holds the `[store, setter]` tuple: `st[0]` is the store proxy and `st[1]` its
setter. Only `st[0].<key>` is a store Get (STRICT_READ_UNTRACKED in a body); `st[0]` alone, `st[1]` or a
dynamic index reads the array and Chrome is silent. What the element is handed to (`const o = st[0]`) is not
followed, so that site is uncertifiable, never a violation. Chrome, rc.21 (patterns/storeindex).
