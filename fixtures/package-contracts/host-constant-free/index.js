import { isServer } from "@solidjs/web";
import * as web from "@solidjs/web";
import { onCleanup } from "solid-js";
//#region src/index.ts
/**
* `onElementConnect` of `@solid-primitives/lifecycle@1.0.0-next.2`
* (`dist/index.js`), byte for byte below this comment: a server guard, a
* conditional value-carrying return, and a cleanup registration.
*/
function onElementConnect(el, fn) {
	if (isServer) return;
	if (el.isConnected) return fn();
	const observer = new ResizeObserver(() => el.isConnected && (observer.disconnect(), fn()));
	observer.observe(el);
	onCleanup(() => observer.disconnect());
}
/**
* The same body under a local `isServer` that shadows the import: the guard
* reads the local binding, which no host decides.
*/
function shadowedGuard(el, fn) {
	const isServer = el.isSameNode(null);
	if (isServer) return;
	if (el.isConnected) return fn();
	const observer = new ResizeObserver(() => el.isConnected && (observer.disconnect(), fn()));
	observer.observe(el);
	onCleanup(() => observer.disconnect());
}
/**
* The same body reading `isServer` through a namespace import, which no
* host constant names.
*/
function namespaceGuard(el, fn) {
	if (web.isServer) return;
	if (el.isConnected) return fn();
	const observer = new ResizeObserver(() => el.isConnected && (observer.disconnect(), fn()));
	observer.observe(el);
	onCleanup(() => observer.disconnect());
}
/**
* A guard with no other early exit, the shape of `createPureReaction`'s: the
* registration runs on every completion wherever the guard never returns.
*/
function guardedCleanup(fn) {
	if (isServer) return;
	onCleanup(fn);
}
//#endregion
export { guardedCleanup, namespaceGuard, onElementConnect, shadowedGuard };
