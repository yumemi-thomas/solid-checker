# `render` in a body drains an apply on the body's stack

ADR 0644, measured in Chrome: the apply runs under the body's owner and its write throws `REACTIVE_WRITE_IN_OWNED_SCOPE`. The apply's
ownerless proof is withheld, so the write is uncertifiable, as under `flush()`.
