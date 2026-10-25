import { renderCard } from "./render";

// A component's body calls the destructured helper too, under its owner.
export function Page() {
  const { open } = renderCard();
  open();
  return <div />;
}
