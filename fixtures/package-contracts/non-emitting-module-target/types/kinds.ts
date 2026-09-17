// A type-only module: two aliases and an interface, and not one statement that
// emits JavaScript. This is `@kobalte/utils@2.0.0-alpha.0`'s `src/types.ts`,
// reached through the same unconditional `"./src/*": "./src/*"` shape.
export type ValidationState = "valid" | "invalid";

export interface RangeValue<T> {
	start: T;
	end: T;
}
