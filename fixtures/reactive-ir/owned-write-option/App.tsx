import { createMemo, createSignal, createStore } from "solid-js";

declare const options: { name: string };
declare const flag: boolean;
declare const rest: { name: string };

// Literal options that do not carry `ownedWrite: true`: the guard applies.
const [plain, setPlain] = createSignal(0);
const [named, setNamed] = createSignal(0, { name: "named" });
const [off, setOff] = createSignal(0, { ownedWrite: false });
createMemo(() => setPlain(plain() + 1));
createMemo(() => setNamed(named() + 1));
createMemo(() => setOff(off() + 1));

// `ownedWrite: true` exempts the signal's setter.
const [on, setOn] = createSignal(0, { ownedWrite: true });
createMemo(() => setOn(on() + 1));

// Options the analysis cannot read may carry `ownedWrite: true`: uncertifiable.
const [fromVariable, setFromVariable] = createSignal(0, options);
const [fromSpread, setFromSpread] = createSignal(0, { ...rest });
const [fromValue, setFromValue] = createSignal(0, { ownedWrite: flag });
createMemo(() => setFromVariable(fromVariable() + 1));
createMemo(() => setFromSpread(fromSpread() + 1));
createMemo(() => setFromValue(fromValue() + 1));

// A store setter never reads `ownedWrite` (rc.17 `dist/dev.js:5847`,
// `storeSetter`, and `createStore$1`, `:5904`), so unreadable options on a
// store leave the write a violation, and so does a variable that names the key.
const storeOptions = { name: "store", ownedWrite: true };
const [store, setStore] = createStore({ value: 0 }, storeOptions);
const [plainStore, setPlainStore] = createStore({ value: 0 });
createMemo(() => setStore((draft) => { draft.value = 1; }));
createMemo(() => setPlainStore((draft) => { draft.value = 1; }));
