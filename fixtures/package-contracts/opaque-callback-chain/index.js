function retain(callback) {
  return { callback };
}

function invoke(callback) {
  callback();
}

export function Opaque(callback) {
  retain(() => invoke(callback));
}

export function Direct(callback) {
  invoke(callback);
}

export function Stored(callback) {
  const run = () => invoke(callback);
  return { run };
}
