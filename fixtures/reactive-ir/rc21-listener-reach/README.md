# What a listener reaches runs on the dispatcher's stack

ADR 0645, measured in Chrome: `STRICT_READ_UNTRACKED`, and a thrown `REACTIVE_WRITE_IN_OWNED_SCOPE`, from an object's method, a bound
member and a `handleEvent` object, on a `window` dispatch in a body. Uncertifiable at the registration.
