function retain(callback) {
  return callback;
}

export function captured(props) {
  retain(() => props.of.values());
}

export function direct(props) {
  props.of.values();
}

export function mixed(props) {
  props.other.values();
  retain(() => props.of.values());
}
