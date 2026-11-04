# after-await-store-paths

**Claim (ADR 0614).** SC1002 reads a store through what the strict read reaches it by: a receiver holding exactly a `const` (`thing.useIt.a`, its keys compared after the root so a key read before the await stays tracked), and a function written before the await and run past it (`wrapUse()`). Measured on rc.17: `Alias`, `Wrapper` and `WrapperInClosure` warn `UNTRACKED_READ_AFTER_AWAIT`; the tracked and before-await cases report nothing.
