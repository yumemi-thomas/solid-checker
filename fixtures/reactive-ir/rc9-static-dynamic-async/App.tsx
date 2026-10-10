// `dynamic(source, { static: true })` with a promise-valued source, on
// @solidjs/web@2.0.0-rc.9. The static form calls the source once, untracked,
// before `dynamic` returns, and needs its component synchronously: the dev
// builds throw "dynamic(): a static source must resolve synchronously, not to
// a promise" (`dist/web.dev.js:2249`, `dist/server.dev.js:3979`), and the
// production builds fall through to a component that renders nothing
// (`dist/web.js:2080-2090`, `dist/server.js:3729-3733`). The source parameter
// is typed `() => T | Promise<T> | ...` whatever `static` says, so every call
// here is `tsc`-clean.
import * as Web from "@solidjs/web";
import { dynamic } from "@solidjs/web";

const Plain = () => <div />;

// --- Positive: the source provably returns a Promise ------------------------

// An async function returns a Promise on every call.
export const AsyncArrow = dynamic(async () => Plain, { static: true });

export const AsyncFunctionExpression = dynamic(async function () {
  return Plain;
}, { static: true });

// Parentheses are transparent.
export const ParenthesizedAsync = dynamic((async () => "span" as const), { static: true });

// The form is chosen after the callee resolves, so a namespace import is the
// same call.
export const NamespaceAsync = Web.dynamic(async () => Plain, { static: true });

// A non-async source whose returned expression the compiler resolves to the
// standard library's Promise constructor.
export const ResolvedPromise = dynamic(() => Promise.resolve(Plain), { static: true });

export const ConstructedPromise = dynamic(
  () => new Promise<typeof Plain>((resolve) => resolve(Plain)),
  { static: true }
);

// A source named by an identifier that resolves to a same-file async function
// declaration, or to a `const` initialized with an async arrow.
async function loadPlain() {
  return Plain;
}
export const NamedAsync = dynamic(loadPlain, { static: true });

const loadTag = async () => "span" as const;
export const ConstAsync = dynamic(loadTag, { static: true });

// --- Negative: the static form with a synchronous source --------------------

export const SyncStatic = dynamic(() => Plain, { static: true });

// --- Negative: the default form settles an async source in its memo ---------

export const AsyncDefault = dynamic(async () => Plain);

export const AsyncFalse = dynamic(async () => Plain, { static: false });

export const AsyncDeferStream = dynamic(async () => Plain, { deferStream: true });

// --- Negative: not proven here ----------------------------------------------

// The options value is not a literal, so either runtime may run: the dialect
// answers the form that states nothing.
export function unknownFlag(flag: boolean) {
  return dynamic(async () => Plain, { static: flag });
}

// A local `Promise` is not the standard library's: this source returns `Plain`
// synchronously.
export function shadowedPromise() {
  const Promise = { resolve: <T,>(value: T) => value };
  return dynamic(() => Promise.resolve(Plain), { static: true });
}

// A caller-supplied source: what it returns is the caller's.
export function fromParameter(load: () => Promise<typeof Plain>) {
  return dynamic(load, { static: true });
}

// A reassignable binding may hold another function by the time of the call.
let loadLater = async () => Plain;
export const LetAsync = dynamic(loadLater, { static: true });
export function replaceLoader() {
  loadLater = async () => Plain;
}

// Approximation: a block-bodied non-async source is not followed, although this
// one does return a Promise.
export const BlockBodiedResolve = dynamic(() => {
  return Promise.resolve(Plain);
}, { static: true });
