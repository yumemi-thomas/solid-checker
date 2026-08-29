# Class-expression export kind

This fixture pins fail-closed proposal generation for bundled class-expression
shapes. A runtime class is `typeof === "function"`, while TypeScript callability
alone reports no call signature; exact constructability is therefore required
to distinguish constructors from ordinary values without syntax guessing.

The stable-v1 generator analyzes each finite entrypoint independently. It
may retain proven local export facts, but it refuses this fixture's package
because its bare external export-all boundary has no independently accepted
dependency semantics. A newly generated dependency proposal cannot be fed back
as proof for the parent proposal, and the generator no longer carries a
same-run or name-only dependency summary.

`expected-refusal.txt` pins that boundary. Focused IR tests separately pin the
kind decision:

- callable or constructable proves runtime function kind;
- closed non-callable plus non-constructable proves value kind;
- unknown, mixed, or absent facts prove neither and refuse the entrypoint;
- raising a value summary to function leaves callback behavior open.

The fixture therefore guards both false negative closure (`class` published as
inert value) and false positive dependency trust. No serialized unknown
sentinel, variant, or inline evidence object is part of the result.
