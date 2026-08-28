# Exported classes

This fixture exercises runtime-kind proof for direct, aliased, default, and
barrel-exported classes. Type Facts derives callability and constructability at
the exact export location; a class is a runtime function even though its type
has no call signature. Direct and anonymous class declarations also use the
parser's exact declaration-span fact because the type at a declaration name is
the instance type, not the constructor value.

A proposal may publish function kind only when callability, constructability,
or exact class-declaration identity proves it. It may publish value kind only
when callability and constructability are both closed negative. Unknown,
mixed, or absent facts remain an open kind premise and refuse that finite
entrypoint; omission is never interpreted as “inert value.” Callback behavior
for a proven constructor stays independently open because function kind does
not prove what its constructor does with callable arguments.

The Phase 14 generator refuses the overall package at an unsupported exported
class surface and records the exact reason in `expected-refusal.txt`. Unit tests
retain the positive/negative kind matrix and the rule that raising function
kind cannot close callbacks. The fixture no longer expects a flat schema-1
summary or inline unknown sentinel.
