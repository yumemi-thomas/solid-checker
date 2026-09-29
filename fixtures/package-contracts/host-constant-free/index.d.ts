declare function onElementConnect(el: Element, fn: VoidFunction): void;
declare function shadowedGuard(el: Element, fn: VoidFunction): void;
declare function namespaceGuard(el: Element, fn: VoidFunction): void;
declare function guardedCleanup(fn: VoidFunction): void;
export { guardedCleanup, namespaceGuard, onElementConnect, shadowedGuard };
