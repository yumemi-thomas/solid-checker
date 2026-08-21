# props-caller-witness

Caller-proven prop reactivity is two questions with opposite quantifiers, and
they need different amounts of evidence. Solid 2.0 compiles a statically
passed prop to a plain property and a dynamically passed one to a getter, so:

- **"some caller passes a reactive expression"** — one witness proves it, and
  the witness is *monotone*: a consumer outside the project can add a call
  site, never unwrite the one written here. Sound in an open world.
- **"every caller passes a static value"** — one unseen caller falsifies it.
  Needs the complete caller set, so it needs a closed world.

Before this fixture an exported component forfeited **both** halves: its
classification collapsed to a single "nothing about its props is provable"
state, which threw away in-project witnesses and reported a proof obligation
where a violation was proven.

| Case | Caller set | Visible witness | Outcome |
| --- | --- | --- | --- |
| `WitnessedDynamic` | open (exported) | dynamic | violation |
| `OnlyStaticWitness` | open (exported) | static only | uncertifiable |
| `ClosedStatic` | complete (local) | static only | silent |
| `WitnessedPerName` | open (exported) | dynamic for `shown`, static for `hidden` | violation on `shown`, uncertifiable on `hidden` |

`OnlyStaticWitness` is the negative control that matters most: a static
witness must never certify an escaping component as silent, because the caller
that passes a signal may live in another package. `ClosedStatic` is the same
code with the caller set closed, and it is the one that goes quiet — so the
discriminator on that row is enumerability, not the value passed.

`WitnessedPerName` pins the granularity. A witness is per prop *name*, not per
component: without that, one dynamic prop would make every prop on the same
component report.
