# Single-call parameter transport

One exact direct call to an unexported function supplies an unwritten plain
parameter with an exact transported object or returned provider array. The
parameter and argument share the existing receiver census, including across
captured query closures. Body/apply and pre-await calls retain the actual
consumer's execution role; tracked consumers stay non-violating.

Second calls, exported functions, function aliases/escapes, reassignment,
default/rest parameters, argument spreads, `arguments` use and parameter
escape refuse this channel. After-await execution remains uncertifiable.
The `input` string handed on in `Apply` and `AsyncPrefix` raises no ADR 0320
unmodeled-use obligation: each effect's apply callback receives what its
compute returns, the literal `"input"` (a proof from the producer, not from
the declared type).
The exported control is also refused for closed apps; library exports retain
the existing ADR 0193 open-program boundary. No type grants source authority.

The reduced declarations copy the neighboring published rc.13 scaffold
unchanged. Real published rc.13 types are checked separately in isolated
temporary copies. ADR 0273's member/name/wildcard stability veto still applies.
