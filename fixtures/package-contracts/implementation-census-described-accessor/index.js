// The tracer for item A of ways-to-improve § 3.3: a `callbacks` enumeration
// whose items include non-call uses of the caller's argument -- a property read
// (`get`) or a coercion (`coerce`) -- which the generator derives from its own
// walk and the implementation census confirms site for site.

// --- Every export in this block certifies its callbacks. ---

// `@solid-primitives/utils@7.0.0-next.4`'s own `access`: a call of the
// argument and a read of its `length`, both at the call.
export const access = (v) => typeof v === "function" && !v.length ? v() : v;

// `@solid-primitives/utils@7.0.0-next.4`'s own `compare`: both arguments
// coerced by `<` and `>`, and nothing called.
export const compare = (a, b) => a < b ? -1 : a > b ? 1 : 0;

// Typed-primitive coercions: under the declared-signature premise the producer
// records no form for a `number` operand, so a `coerce` item the walk derives
// for one finds no use and narrows out of the closed enumeration.
export function plainArithmetic(a, b) {
  return a + b;
}

// A call of parameter 0 beside a coercion of the number-typed parameter 1:
// only the coercion narrows; the call stays described.
export const callAndAdd = (f, n) => f() + n;

// `@solid-primitives/utils`' own `isNonNullable`: a loose comparison with
// `null` applies ToPrimitive to nothing, so nothing is derived.
export const isNonNullable = (i) => i != null;

// --- Every export below is refused. ---

// A read deferred into a returned closure: not at the call.
export const deferredRead = (v) => () => v.length;

// A read inside a nested callable the body then calls: still a nested frame.
export const nestedRead = (v) => {
  const read = () => v.length;
  return read();
};

// A read of a parameter whose value may be the default: not the caller's own
// value at that slot.
export const defaultRead = (v, w = v) => w.length;

// A read and a coercion of the same argument.
export const readAndCoerce = (v) => v.length + v;

// An array destructuring of one argument beside a read of another's length.
// Typed as tuples and arrays, the destructuring records no form under the
// declared-signature premise -- the engine's own array iterator is presumed --
// but a caller-supplied value of that type may be a Proxy, whose traps the
// destructuring runs: an iteration no item describes.
export function destructureAndMeasure(point, polygon) {
  const [x, y] = point;
  return x + y + polygon.length;
}

// `@kobalte/utils@2.0.0-alpha.0`'s own `isPointInPolygon`, the case the
// ecosystem census found: it iterates `point` and each `polygon[i]`.
export function isPointInPolygon(point, polygon) {
	const [x, y] = point;
	let inside = false;
	const length = polygon.length;
	for (let l = length, i = 0, j = l - 1; i < l; j = i++) {
		const [xi, yi] = polygon[i];
		const [xj, yj] = polygon[j];
		const [, vy] = polygon[j === 0 ? l - 1 : j - 1] || [0, 0];
		const where = (yi - yj) * (x - xi) - (xi - xj) * (y - yi);
		if (yj < yi) {
			if (y >= yj && y < yi) {
				if (where === 0) return true;
				if (where > 0) if (y === yj) {
					if (y > vy) inside = !inside;
				} else inside = !inside;
			}
		} else if (yi < yj) {
			if (y > yi && y <= yj) {
				if (where === 0) return true;
				if (where < 0) if (y === yj) {
					if (y < vy) inside = !inside;
				} else inside = !inside;
			}
		} else if (y === yi && (x >= xj && x <= xi || x >= xi && x <= xj)) return true;
	}
	return inside;
}
