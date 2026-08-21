# rendering-csr-selected

One semantic claim, pinned for two consumers: **an explicit client-only
rendering selector proves the server-rendering premise false**, and a rule
whose claim is conditioned on "if this application server-renders" must then
be silent rather than report a proof obligation the user has already
discharged.

The selector is `.solid-checker/runtime.json`'s `"rendering": "csr"`. Before
this fixture the analyzer had only two states for the fact — *server
rendering is proven* and *not proven* — and folded "the user selected CSR"
into the second one, so selecting CSR produced an uncertifiable result whose
own message read "the analyzed project cannot prove whether a
server-rendering entry exists". It could: the user had said so.

Three fixtures cover the three states of the same fact:

| Fixture | Premise | Outcome |
| --- | --- | --- |
| `ssr-client-boundary` | server rendering proven (selector, or a visible `renderToStream` import) | violation |
| `ssr-client-boundary-csr`, `http-response-flush-csr` | unresolved — no selector, no visible server entry | uncertifiable |
| this one | proven client-only — explicit `rendering: "csr"` | silent |

The middle row is why absence of a server entry cannot be the discriminator:
`ssr-client-boundary-csr` has no server entry either and still reports, because
the entry may live in another tsconfig or package.

`LoudProfile` is the positive control. SC5003's *async* arm does not depend on
the rendering premise at all — a pending async accessor rendered with no
`Loading` boundary above it still shows nothing while loading under CSR — so
it is still reported here. Without it the fixture could pass by containing
nothing analyzable. Dropping the selector turns the two quiet cases into three
uncertifiable findings and leaves this one unchanged.
