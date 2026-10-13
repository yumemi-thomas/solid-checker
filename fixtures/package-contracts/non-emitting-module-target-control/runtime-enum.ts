// A non-`declare` enum emits an object at runtime, `const enum` included: the
// inlining is a compiler option rather than a property of these bytes.
export enum Direction {
	Up = "up"
}
