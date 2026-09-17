const map = new Map();
const set = new Set();
const array = [];
export function MapStore(value) { map.set("key", value); }
export function SetStore(value) { set.add(value); }
export function ArrayStore(value) { array.push(value); }
export function Direct(callback) { callback(); }
