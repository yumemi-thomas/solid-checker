# iteration-written-slots

**Claim (ADR 0349, ADR 0803).** A program that writes the array iterator visibly (`patch.ts`) has no exact array pattern of a primitive: `const [s, set] = createSignal(1)` may hold anything the patched iterator hands it. `body-write`'s `set(2)` and `s()` are uncertifiable (SC9012, `tuple-iteration-written`), never SC2001. Chrome (rc.21): quiet, the patch hands back a no-op setter (pattern `types/s04-local-setter-iteration-written`). Without `patch.ts` the same body is a proved SC2001 (Chrome REACTIVE_WRITE_IN_OWNED_SCOPE), as every other fixture's tuple still is.

Variants (each `patch.ts` beside the same body):
- `next-alias`: `IP.next = f` through `const IP = Object.getPrototypeOf([][Symbol.iterator]())`, which ADR 0349's spelling misses. Uncertifiable; Chrome quiet.
- `reflect-alias`: `Reflect.set(AP, Symbol.iterator, f)` through `const AP = Array.prototype`. Uncertifiable; Chrome quiet.
- `class-prototype` (negative): `Foo.prototype.bar = 1` on a project class spells `prototype`, and ADR 0282's census clears it. Proved SC2001; Chrome warns.
- `named-key` (negative): `Object.assign(Array.prototype, { zzz() {} })` writes a literal key that is not iteration's. Proved SC2001; Chrome warns.
- `key-alias`: `AP[k] = f` through `const AP: any = Array.prototype`, a key no literal names. Uncertifiable; Chrome quiet.
- `hook-object`: the slots leave a hook in an object (`return { count, setCount }`); the call `setCount(2)` in the body is uncertifiable at the call, not only at the hook's reference.
- `prop-value`: a caller passes a withdrawn slot's read as a prop (`value={level()}`, `value={state.x}`) and the child reads it in its body (`createSignal(props.value)`). The withdrawn slot stays a source, so the reads keep their rows, but the prop's backing is unproven: SC1001 uncertifiable, `read-backing-unproven`, never SC1001 violation from a slot the patch may have replaced. Chrome warns here (this patch replaces only setters): uncertifiable, not wrong.
- `prop-value-intact` (negative): the same without `patch.ts`. Proved SC1001 twice.
- `open-compute`: a memo whose compute calls a withdrawn slot. The slot may be any function, so the memo's provenance is open (ADR 0665): SC5003 uncertifiable, `async-provenance-unproven`, at the JSX read.
- `open-compute-intact` (negative): the same without `patch.ts`. Certified.
- `function-escaped`: a `Function` held in a parameter runs a string that patches iteration (pattern `types/s03`). No write spells it; ADR 0801's census (code from a string) withdraws the slots. Uncertifiable; Chrome quiet.
- `compared-prototype` (negative): `!(proto = Object.getPrototypeOf(obj)) || proto === Object.prototype`, a prototype only compared, as router and store code writes it. No intrinsic is handed on: proved SC2001; Chrome warns.
- `uncalled-accessor` (review a9fc #7): `${count}` of a withdrawn slot. upstream_compat's uncalled-accessor arm takes `count` for the primitive's accessor; the static row is held by place: SC1005 uncertifiable, `tuple-iteration-written`. `uncalled-accessor-intact` (negative): proved SC1005.
- `global-alias-hook` (review a9fc #1): the patch reaches `Array.prototype` through `const g = globalThis; g.Array.prototype` under a key no literal names, and the hook returns a 3-tuple the patch reorders (Chrome: `now` is the setter, REACTIVE_WRITE_IN_OWNED_SCOPE). The global object handed on withdraws the hook's slot: `now()` has its own row, uncertifiable.
