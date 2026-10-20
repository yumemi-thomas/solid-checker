# literal-property-dispatch

**Claim (ADR 0321).** A function literal that is exactly the value of a static-keyed property of an object literal written as a call's argument is that property's implementation (`useThing({ onClose: () => ... })` dispatches `options.onClose()` to the arrow; identity is its file and span; reads stay attributed where written): no `SC9012`; receiver-using functions, later spreads, hand-on to a sink, conditional alternatives and `const`-bound literals (ADR 0320 census) are uncertifiable.
