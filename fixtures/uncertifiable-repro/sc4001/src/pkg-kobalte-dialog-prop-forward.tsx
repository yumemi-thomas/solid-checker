// viviana-ui-web advanced-sections.tsx:174 (callee 1235-1245): a prop is read in a callback given to a project component that calls it from a callback of a package component.
import { Dialog } from "@kobalte/core/dialog";

function DialogDemo(props: { onOpenChange?: (open: boolean) => void }) {
  return (
    <Dialog
      onOpenChange={(open) => {
        if (!open) props.onOpenChange?.(open);
      }}
    >
      <Dialog.Trigger>Open</Dialog.Trigger>
    </Dialog>
  );
}

export function Playground(props: { onLastAction: (message: string) => void }) {
  return <DialogDemo onOpenChange={(open) => props.onLastAction(`Dialog open: ${open}`)} />;
}
