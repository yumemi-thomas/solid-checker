# Callback forwarded into an unresolved callee

`mapParam` forwards its callback into `list.map(...)`, where `list` is one of
its own parameters. In a published JavaScript runtime artifact that parameter
is `any`, so the member callee resolves to no declaration at all -- neither an
analyzed implementation nor a standard-library position.

The generator must record that as `callbacks: {"status": "unknown"}`. Omitting
the field is a *negative* claim ("this export never invokes a caller-supplied
callback"), which is false here and, once reviewed, makes a consumer certify a
read inside the callback against the wrong execution timing.

`noMember` is the negative control: it invokes no member on a parameter and
forwards nothing, so it keeps a clean summary. The marker must be scoped to the
export whose callback path is actually unproven, never applied to the whole
entrypoint.
