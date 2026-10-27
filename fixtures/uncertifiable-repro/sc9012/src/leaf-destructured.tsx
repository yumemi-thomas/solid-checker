// donegeon-client BoardControllerDeckEffects.tsx:153: a leaf callback calls an
// accessor destructured from the controller's context parameter, which the
// caller builds by spreading its hooks' results.
import { createSignal, createTrackedEffect } from "solid-js";

type DeckContext = { deckOrderPrefs: () => string[]; setDeckHubOpen: (open: boolean) => void };

function createDeckEffects(context: DeckContext) {
  const { deckOrderPrefs } = context;
  createTrackedEffect(() => {
    const prefs = deckOrderPrefs();
    if (prefs.length === 0) window.localStorage.removeItem("deck-row-prefs");
  });
}

export function Board() {
  const [prefs] = createSignal<string[]>([]);
  const [hub, setDeckHubOpen] = createSignal(false);
  const deck = { deckOrderPrefs: prefs };
  createDeckEffects({ ...deck, setDeckHubOpen });
  return <p>{hub() ? prefs().length : 0}</p>;
}
