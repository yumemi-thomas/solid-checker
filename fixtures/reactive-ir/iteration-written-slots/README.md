# iteration-written-slots

**Claim (ADR 0349, ADR 0803).** A program that writes the array iterator visibly (`patch.ts`) has no exact array pattern of a primitive: `const [s, set] = createSignal(1)` may hold anything the patched iterator hands it. `body-write`'s `set(2)` and `s()` are uncertifiable (SC9012, `tuple-iteration-written`), never SC2001. Chrome (rc.21): quiet, the patch hands back a no-op setter (pattern `types/s04-local-setter-iteration-written`). Without `patch.ts` the same body is a proved SC2001 (Chrome REACTIVE_WRITE_IN_OWNED_SCOPE), as every other fixture's tuple still is.

Variants (each `patch.ts` beside the same body):
- `next-alias`: `IP.next = f` through `const IP = Object.getPrototypeOf([][Symbol.iterator]())`, which ADR 0349's spelling misses. Uncertifiable; Chrome quiet.
- `reflect-alias`: `Reflect.set(AP, Symbol.iterator, f)` through `const AP = Array.prototype`. Uncertifiable; Chrome quiet.
- `class-prototype` (negative): `Foo.prototype.bar = 1` on a project class spells `prototype`, and ADR 0282's census clears it. Proved SC2001; Chrome warns.
- `named-key` (negative): `Object.assign(Array.prototype, { zzz() {} })` writes a literal key that is not iteration's. Proved SC2001; Chrome warns.
- `key-alias`: `AP[k] = f` through `const AP: any = Array.prototype`, a key no literal names. Uncertifiable; Chrome quiet.
- `hook-object`: the slots leave a hook in an object (`return { count, setCount }`); the call `setCount(2)` in the body is uncertifiable at the call, not only at the hook's reference.
