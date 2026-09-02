# A member read is an obligation only when it reaches a parameter

`drop(list, count)` reads `list.slice`. `list` comes from the caller, so the
read is externally visible and the summary carries one read operation.

`readModuleLocal` and `readBodyLocal` perform the identical `.slice` member
read, but on an array this module created -- a module-scope constant and a
function-body local. Nothing a consumer supplies is read, so neither export
carries a read.

Promoting every member expression would attach a read obligation to the two
locals as well, which is a claim about the consumer's values that this package
cannot make. `parameter-member-forwarded` pins the case where the parameter
reaches the member through another call instead of directly.
