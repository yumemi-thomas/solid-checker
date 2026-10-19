import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Uncertifiable: stored in an object and through an alias, then called as
// plain functions inside tracked JSX (runtime-probed: no
// STRICT_READ_UNTRACKED).
function Badge() {
  const value = count();
  return <b>{value}</b>;
}

function Chip() {
  const value = count();
  return <b>{value}</b>;
}

// Positive: `Inner` is rendered by its tag, a component however `Shell` is
// entered (runtime-probed: STRICT_READ_UNTRACKED names <Inner>).
function Shell() {
  function Inner() {
    const value = count();
    return <i>{value}</i>;
  }
  return <Inner />;
}

const parts = { badge: Badge, shell: Shell };
const chip = Chip;

export function Panel() {
  return <p>{parts.badge()}{[1].map(chip)}{parts.shell()}</p>;
}
