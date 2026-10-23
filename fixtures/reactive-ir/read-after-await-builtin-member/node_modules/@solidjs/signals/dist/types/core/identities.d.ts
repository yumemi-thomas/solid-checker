/** @internal The identity of a plain signal named while identities were on. */
export declare function signalName(node: object | undefined): string | undefined;
/** @internal Name plain signals from now on (`on`), or stop. */
export declare function setSignalIdentities(on: boolean): void;
/**
 * @internal Called with every plain signal `createSignal` makes, and every
 * plain store root `createStore` makes, while set (the namer above). Never
 * set unless `setSignalIdentities` is imported, so a bundle without it keeps
 * only a null check.
 */
export declare let signalHook: ((node: object) => void) | null;
/**
 * @internal Dev only: `watch` is called with every node read while set (the
 * dev hook `onRead`) — the frames client's identity-drift check watches the
 * named signals a deferred occurrence reads as it mounts. A no-op in prod.
 */
export declare function setReadWatch(watch: ((node: object) => void) | null): void;
