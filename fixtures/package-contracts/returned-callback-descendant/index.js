export function Direct(callback) {
  return () => callback();
}

export function Dormant(callback) {
  return () => {
    function mapper() { callback(); }
    return 1;
  };
}

export function Invoked(callback) {
  return () => {
    function mapper() { callback(); }
    mapper();
  };
}
