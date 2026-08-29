# A non-literal `import()` keeps only the affected claims open

`load` returns ``import(`./mod-${name}.js`)``. The substitution prevents exact
static resolution: neither artifact acquisition nor compiler facts can name
the runtime module selected by an arbitrary `name`.

The stable-v1 proposal still retains the exact `index.js` artifact case and
the independently known export surface. It emits no closure candidate for the
unbounded dynamic-loading behavior and leaves the affected call/value domains
open. That refusal does not erase the unrelated `thing` export or turn absence
of a recorded module into complete-negative knowledge.

`mod-a.js` remains on disk deliberately. It must not enter the recorded closure
merely because it exists; the analyzed program never selected it. Making the
specifier literal changes the premise and should make this fixture fail.

`expected.json` pins the stable-v1 main document. `expected-proposal.json`
pins the claim-addressed proof plan and, critically, its empty
`closureCandidates` set for this unbounded leaf. Acceptance still requires the
ordinary proof-and-receipt workflow; generator coverage is not authority.
