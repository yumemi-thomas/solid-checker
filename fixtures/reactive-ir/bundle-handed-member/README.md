# bundle-handed-member

**Claim (ADR 1207).** A literal used only through keys it owns is confined though `Object.prototype` may be written (`package_hand_off::own_key_confined`): a plain `const` literal with no spread, each property a data property holding a function that names no receiver, not exported, every reference of it the object of a member under one of its own keys that nothing writes or deletes, read (it gives out the function) or called as a method of an inert callee. No accessor a prototype may hold runs against it, and no function reads it as a receiver, so a slot's function handed whole to a scheduler (`setTimeout(cb1.cb)`) is never run by a package. The program names no action, deferring primitive or leaf scope, so such a flush ends by the program censuses.

Clean: `HandedToTimer`, `HandedAndCalled`. Uncertifiable: `ObjectHanded` (the object handed to a function nothing reads), `ForeignKeyRead` (a key the literal does not own).

Stub: `member-call-entries`'s. Chrome: `runtime-oracle/bundle-handed-member`.
