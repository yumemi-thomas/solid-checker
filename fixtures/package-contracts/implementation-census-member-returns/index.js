// The tracer for item B round 2 of ways-to-improve § 3.3: a `returns`
// enumeration for "a property of the caller's argument, or undefined", which
// the generator derives from its own walk and the implementation census
// decides from the producer's return arms.

// --- Every export in this block certifies its returns. ---

// `@kobalte/utils@2.0.0-alpha.0`'s own `callHandler`, byte for byte: the
// event's `defaultPrevented` when the event is not nullish, and undefined
// when it is.
function callHandler(event, handler) {
	if (handler) if (typeof handler === "function") handler(event);
	else handler[0](handler[1], event);
	return event?.defaultPrevented;
}

// One member, and nothing else.
export function readKey(options) {
	return options.key;
}

// A literal-keyed element access, optional: member `0`, or undefined.
export function firstOrUndefined(list) {
	return list?.[0];
}

// A string key is the property it spells.
export function stringKey(options) {
	return options["run"];
}

// A member beside the argument itself.
export function keyOrSelf(value, useKey) {
	return useKey ? value.key : value;
}

// The member written before the return reads it is still the member the
// return reads: the claim is what the argument holds there at return time.
export function writtenMember(options) {
	options.key = 1;
	return options.key;
}

// --- Every export below is refused. ---

// A computed key that is not a literal names no member.
export function computedKey(options, key) {
	return options[key];
}

// A written binding: the member read need not be the caller's.
export function writtenBinding(options) {
	options = options || {};
	return options.key;
}

// A path two segments deep: a returned member is described one deep.
export function longerPath(options) {
	return options.inner.key;
}

// A call of the member hands back what the call returned, not the member.
export function memberCall(options) {
	return options.key();
}

// The value read before the write is not what the argument holds at return.
export function readBeforeWrite(options) {
	const before = options.key;
	options.key = 1;
	return before;
}

// No optional chain: `undefined` is not a value this return hands back.
export function overclaimedUndefined(options) {
	return options.key;
}

export { callHandler };
