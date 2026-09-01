export function Parameter(onData) {
  return onData(1);
}

export function ObjectPattern(props) {
  const { onData } = props;
  return onData(1);
}

export function MemberAlias(props) {
  const onData = props.onData;
  return onData(1);
}

export function ArrayPattern(handlers) {
  const [onData] = handlers;
  return onData(1);
}
