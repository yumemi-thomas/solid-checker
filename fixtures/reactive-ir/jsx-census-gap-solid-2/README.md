# JSX census gaps (Solid 2)

Absence of a compiler execution fact is not proof of untracked execution or deletion: a template-root void child is reported discarded and silent; a static `<noscript>` child has no usable execution site (uncertifiable SC1001); the child beside a dynamic `textContent` attribute is tracked (SC8003 reports the conflict); body reads outside JSX are proven violations. The fixture follows the pinned `solidjs/solid` compiler semantics; the checker consumes positive execution facts and fails closed over missing ones.
