// app-game solid-svg-editor svg-renderer.tsx:55 (viviana-ui-web advanced-sections.tsx ListBoxDemo): a prop is read in a callback written in a component and given to a package component.
import { Dialog } from "@kobalte/core/dialog";

export function Picker(props: { onPick: (message: string) => void }) {
  return (
    <Dialog
      onOpenChange={(open) => {
        if (open) props.onPick("opened");
      }}
    >
      <Dialog.Trigger>Open</Dialog.Trigger>
    </Dialog>
  );
}
