const text = "a";

export function convertNumber(raw) {
  return Number(raw);
}

export function convertBoolean(raw) {
  return Boolean(raw);
}

export function convertBigInt(raw) {
  return BigInt(raw);
}

export function convertSymbol(raw) {
  return Symbol(raw);
}

export function convertObject(raw) {
  return Object(raw);
}

export function constructArray(raw) {
  return new Array(raw);
}

export function arrayFrom(items, mapper) {
  return Array.from(items, mapper);
}

export function typedArrayFrom(items, mapper) {
  return Uint8Array.from(items, mapper);
}

export function int8ArrayFrom(items, mapper) {
  return Int8Array.from(items, mapper);
}

export function uint8ClampedArrayFrom(items, mapper) {
  return Uint8ClampedArray.from(items, mapper);
}

export function int16ArrayFrom(items, mapper) {
  return Int16Array.from(items, mapper);
}

export function uint16ArrayFrom(items, mapper) {
  return Uint16Array.from(items, mapper);
}

export function int32ArrayFrom(items, mapper) {
  return Int32Array.from(items, mapper);
}

export function uint32ArrayFrom(items, mapper) {
  return Uint32Array.from(items, mapper);
}

export function float32ArrayFrom(items, mapper) {
  return Float32Array.from(items, mapper);
}

export function float64ArrayFrom(items, mapper) {
  return Float64Array.from(items, mapper);
}

export function bigInt64ArrayFrom(items, mapper) {
  return BigInt64Array.from(items, mapper);
}

export function bigUint64ArrayFrom(items, mapper) {
  return BigUint64Array.from(items, mapper);
}

export function replace(replacement) {
  return text.replace("a", replacement);
}

export function replaceAll(replacement) {
  return text.replaceAll("a", replacement);
}

export function observeReporting(callback) {
  return new ReportingObserver(callback);
}

export function observeIntersection(callback) {
  return new IntersectionObserver(callback);
}

export function getPosition(success, error) {
  return navigator.geolocation.getCurrentPosition(success, error);
}

export function watchPosition(success, error) {
  return navigator.geolocation.watchPosition(success, error);
}

export function postTask(callback) {
  return scheduler.postTask(callback);
}

export function retainArray(value) {
  const values = /** @type {unknown[]} */ ([]);
  return values.push(value);
}

export function retainSet(value) {
  const values = new Set();
  return values.add(value);
}

export function retainMap(value) {
  const values = new Map();
  return values.set("key", value);
}

export function constructSet(values) {
  return new Set(values);
}

export function constructMap(values) {
  return new Map(values);
}

export function constructWeakSet(values) {
  return new WeakSet(values);
}

export function constructWeakMap(values) {
  return new WeakMap(values);
}

function String(raw) {
  return raw;
}

export function shadowedString(raw) {
  return String(raw);
}

function queueMicrotask(value) {
  return value;
}

export function shadowedQueueMicrotask(raw) {
  return queueMicrotask(raw);
}
