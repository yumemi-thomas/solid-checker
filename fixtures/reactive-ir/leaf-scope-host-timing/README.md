# Audited host callback timing (HT)

Leaf-rule expectations with default-library host types: a `Promise` executor and populated `NodeList` iteration invoking `register` synchronously are SC3001 violations; empty/attribute-only bodies, media/global listener registration, existing observer rows and `handleEvent` objects are clean; unfollowed wrappers, same-named project methods, shadowed globals, unspecialized `resolve` and synchronous `dispatchEvent` (ADR 0329) are SC9012. Exact synchronous dispatch stays unmodeled (ADR 0210). Stub byte-identical to `leaf-scope-host-callbacks`'.
