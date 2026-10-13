// Bound owned evidence history by record count and accounted UTF-8 bytes.
// This does not bound the application's heap or prove complete execution coverage.
const encoder = new TextEncoder();
export const encodedEvidenceBytes = value => encoder.encode(typeof value === 'string' ? value : JSON.stringify(value)).length;
export function evidenceLimit(value, fallback, name) {
  value ??= fallback;
  if (!Number.isSafeInteger(value) || value < 1) throw new RangeError(`${name} must be a positive safe integer`);
  return value;
}
export function boundedEvidenceRecords({maxRecords, maxBytes, onRemove = () => {}}) {
  maxRecords = evidenceLimit(maxRecords, null, 'maxRecords');
  maxBytes = evidenceLimit(maxBytes, null, 'maxBytes');
  const records = new Map();
  let bytes = 0, evicted = 0, refused = 0;
  function remove(key) {
    const record = records.get(key);
    records.delete(key); bytes -= record.bytes; evicted++;
    onRemove(key, record.value);
  }
  return {
    get size() { return records.size; },
    get stats() { return {records: records.size, bytes, maxRecords, maxBytes, evicted, refused}; },
    get(key) {
      const record = records.get(key);
      if (!record) return undefined;
      records.delete(key); records.set(key, record);
      return record.value;
    },
    set(key, value, weight = encodedEvidenceBytes(key) + encodedEvidenceBytes(value)) {
      if (!Number.isSafeInteger(weight) || weight < 0) throw new RangeError('Evidence weight must be a nonnegative safe integer');
      if (weight > maxBytes) { refused++; return false; }
      if (records.has(key)) remove(key);
      while (records.size >= maxRecords || bytes + weight > maxBytes) remove(records.keys().next().value);
      records.set(key, {value, bytes: weight}); bytes += weight;
      return true;
    },
  };
}
