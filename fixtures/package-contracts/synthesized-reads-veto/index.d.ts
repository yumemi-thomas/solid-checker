// Every export takes one number: the veto samples it with `1`, a slot its
// value facts describe in full, which is the only kind ADR 0163 synthesizes.
export declare function readsNothing(step: number): number;
export declare function readsOwnSignal(step: number): number;
export declare function readsCreatedSignal(step: number): number;
export declare function readsCreatedMemo(step: number): number;
export declare function readsThenThrows(step: number): never;
export declare function throwsWithoutReading(step: number): never;
export declare function readsUntracked(step: number): number;
export declare function readsLater(step: number): number;
export declare function createsAReadingMemo(step: number): number;
