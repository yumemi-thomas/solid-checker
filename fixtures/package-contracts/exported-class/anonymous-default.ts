// An anonymous default-exported class. There is no name span to record, so the
// export carries the `class …` node's own span — where the compiler's type is
// the class's *instance* type, honestly `nonCallable` and `nonConstructable`.
// `typeof (await import(…)).default === "function"` all the same.
export default class {}
