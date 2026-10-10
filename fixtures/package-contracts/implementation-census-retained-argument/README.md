# A constructor that keeps its caller's callable

Pins ADR 0139. A class export is constructed (ADR 0105), and a construction
may keep a callable its caller hands it on the instance, for the instance's
own members to call later. The `result-access` event states exactly that: the
export stores the callable only in the value it returns, and it runs on the
stack of whoever invokes it through that value.

Expected generation:

| Export | `callbacks` | Why |
| --- | --- | --- |
| `Keeper` | closed, `result-access` from 0 | stored once as `this.callback = callback`, called only by `run`, which nothing at construction reaches |
| `Primed` | open | the constructor also calls `callback(0)`; the generator derives no call item for a class, so it describes nothing (the census confirms the pair when it is stated by hand) |
| `PrimedUndescribed` | open | the same bytes, kept for the census test that states only the kept item |
| `RunsAtConstruction` | open | the constructor calls `this.run(0)`, which calls the key |
| `HandsOn` | open | `later` reads the key without calling it: the callable is handed on |
| `Escapes` | open | `registry.push(this)`: code that never received the instance reaches it |
| `Rewritten` | open | `reset` writes the key again |
| `Augmented` | open | `Augmented.prototype.run = …` is a reference to the class the walk does not read |
| `KeeperUnkept` | open | nothing is kept |

The certification half is
`contract_certification::tests::the_retained_argument_census_certifies_exactly_what_members_keep`,
which states the claims by hand: `Keeper`'s item and `Primed`'s call-plus-kept
pair certify through the Type Facts producer's retained-argument census
(handshake protocol 65), the use census and the constructed veto; every other
export withholds by name. The producer's own census is pinned row by row by
`TestAConstructionStatesWhatItKeepsForItsMembers`, and the generator's by
`retained_arguments::tests`.
