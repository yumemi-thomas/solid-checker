# Audited host callback timing (HT)

Leaf-rule expectations with default-library host types: a `Promise` executor invoking `register` synchronously is an SC3001 violation, `NodeList` iteration an SC3001 obligation (ADR 0563: its length is not read, so it may run nothing); empty/attribute-only bodies, media/global listener registration, existing observer rows and `handleEvent` objects are clean; same-named project methods, shadowed globals, unspecialized `resolve` and synchronous `dispatchEvent` (ADR 0329) are SC9012. Exact synchronous dispatch stays unmodeled (ADR 0210). Stub byte-identical to `leaf-scope-host-callbacks`'.

ADR 0538: a callback behind TypeScript sugar (`register satisfies () => void`) resolves through its declaration, so WrappedNodeListCallback is judged as the unwrapped form is (an obligation since ADR 0563).
