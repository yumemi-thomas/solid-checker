# top-window-local

**Claim (ADR 0906), negative twin of `top-window-patch`.** A `const top` of the module is no window: writing its `setTimeout` patches nothing, and the program keeps its standard timer.
