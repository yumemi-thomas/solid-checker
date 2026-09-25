// The tracer for item B of ways-to-improve § 3.3: a `callbacks` item for a
// call of a literal-keyed member of the caller's argument, which the generator
// derives from its own walk and the implementation census confirms site for
// site.

// --- Every export in this block certifies its callbacks. ---

// `@kobalte/utils@2.0.0-alpha.0`'s own `callHandler`, byte for byte: a call of
// the argument, a call of its member 0, reads of its members 0 and 1, and a
// read of the event's `defaultPrevented`.
function callHandler(event, handler) {
	if (handler) if (typeof handler === "function") handler(event);
	else handler[0](handler[1], event);
	return event?.defaultPrevented;
}

// The member call alone: `[fn, data]` is always a bound handler here.
export function callBound(event, handler) {
	handler[0](handler[1], event);
}

// --- Every export below is refused. ---

// `@kobalte/utils@2.0.0-alpha.0`'s own `composeEventHandlers`: the member call
// happens in `callHandler`, reached from a returned closure, so it is neither
// at the call event nor in this export's own frame.
function composeEventHandlers(handlers) {
	return (event) => {
		for (const handler of handlers) callHandler(event, handler);
	};
}

// A computed key that is not a literal names no member: the producer roots
// nothing, and the callee is unresolved.
export function computedKey(h, k) {
	h[k]();
}

// A member call deferred into a returned closure: not at the call.
export function deferredMember(h) {
	return () => h[0]();
}

// A written binding: the member called need not be the caller's.
export function writtenBinding(h) {
	h = h || [() => {}];
	h[0]();
}

// A string key is the property it spells, and the census confirms it; the
// synthesized veto installs recording members at indices only, so this
// closure has no module to be vetoed by and is withheld for want of a recipe.
export function stringKey(h) {
	h["run"]();
}

export { callHandler, composeEventHandlers };
