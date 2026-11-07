# impostor-host-proto

ADR 0874, review D7: `document` keeps no member a new prototype replaces, and no key names it: `Object.setPrototypeOf` is a writer of any
key, as is a `__proto__` write. `self` itself is writable. Each receiver stays uncertifiable, `standard-member-impostor`.
