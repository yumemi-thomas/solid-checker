// lib.es5.d.ts's signature-less `Function` family. These types deliberately
// have no readable call signature, but every value admitted by them is still a
// JavaScript function. Type Facts schema 15 answers `UntypedCallable` for the
// family so consumers can prove runtime kind without inventing parameters or
// arity.
type Handler = Function;
interface ExtendedHandler extends Function {}

declare const raw: Function;
declare const callable: CallableFunction;
declare const newable: NewableFunction;
declare const alias: Handler;
declare const extended: ExtendedHandler;
declare const branded: Function & { readonly brand: "route" };

// Negative boundary controls. A function can be assigned to these broad
// types, but so can non-function values; their declared type therefore remains
// honestly `NonCallable` + `NonConstructable` and publishes `kind: "value"`.
declare const bag: object;
declare const empty: {};
declare const table: Record<string, unknown>;
declare const retries: number;

export { alias, bag, branded, callable, empty, extended, newable, raw, retries, table };
