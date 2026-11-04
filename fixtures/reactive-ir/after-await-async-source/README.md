# after-await-async-source

**Claim (ADR 0614).** SC1002 proves no read of an async source: rc.17 warns `UNTRACKED_READ_AFTER_AWAIT` when it is settled and throws, silently here, when it is pending (both measured), so which one the read meets is an obligation.
