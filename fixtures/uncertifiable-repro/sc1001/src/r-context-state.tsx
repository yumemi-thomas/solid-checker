// app-game components ui/popover.tsx:40-41: a context value object whose member reads a prop
// (`open: () => props.open ?? …`).
import type { JSX } from "@solidjs/web";
import { createContext, useContext } from "solid-js";

type PopoverState = { open: () => boolean };

const PopoverContext = createContext<PopoverState>();

export function Popover(props: { open?: boolean; children?: JSX.Element }) {
  const state: PopoverState = { open: () => props.open ?? false };
  return <PopoverContext value={state}>{props.children}</PopoverContext>;
}

export const usePopover = (): PopoverState => useContext(PopoverContext);
