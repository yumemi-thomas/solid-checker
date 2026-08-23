import { mapPath, mapValue } from "reactive-package";

// (a) An inline function literal plus an `accessor` descriptor is the one
// shape source discovery can bind: the claim lands on `item`, which becomes a
// reactive accessor, and the call reports no unbound-claim obligation.
export function inlineLiteral() {
  mapValue((index, item) => item());
}

// (a2) The literal never names the described argument, so the claim has
// nothing it could change about this callback's body. That is a proof, not a
// gap, and the call stays clean.
export function inlineLiteralIgnoringTheArgument() {
  mapValue(index => index);
}

function named(index: number, item: () => number) {
  return item();
}

// (b) The same contract claim with the callback passed by name. Nothing binds
// the descriptor to `item`, so the call must stay uncertifiable instead of
// being analyzed as if the contract had said nothing about its arguments.
export function byName() {
  mapValue(named);
}

// (c) A `store-path` descriptor on a parameter the literal does declare. The
// claim is schema-valid, the consumer does not materialize that shape, and the
// call therefore stays uncertifiable rather than silently clean.
export function storePathDescriptor() {
  mapPath(state => state.value);
}

// (d) A rest parameter is not one of the literal's declared parameters, yet it
// absorbs the described argument: `args[0]` *is* the store path the descriptor
// names. A parameter list shorter than the descriptor row is therefore no proof
// of blindness, and the call fails closed like (c).
export function restParameterAbsorbsTheDescriptor() {
  mapPath((...args) => args[0].value);
}

// (e) A non-arrow literal declares no parameter at all and still reads the
// described argument through its `arguments` object. Same fail-closed
// obligation.
export function argumentsObjectObservesTheDescriptor() {
  mapPath(function () {
    return arguments[0].value;
  });
}
