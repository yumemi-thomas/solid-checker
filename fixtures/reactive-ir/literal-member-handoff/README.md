# Literal member hand-off

ADR 0286: a member read like `tag.label()` admitted by the type-only fallback (the member is typed `Accessor<T>`) is a proven SC1001 only while no code can have replaced that member; every read of the member's property symbol is judged by its receiver (since ADR 0294, positives are proven by what the literal stores). Non-literal receivers are `SC9012`; hand-offs, aliases, assertion writes, `this` hand-off and generic replacement are silent. Stub `context-member-identity`'s (published rc.13).
