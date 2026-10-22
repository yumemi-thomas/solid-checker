import { Dialog, useDialogContext } from "@kobalte/core/dialog";

// Throws NoOwnerError when the button is clicked: an event handler has no
// owner, and the package hook reads the dialog's context in it (ADR 0352).
export function Clicked() {
  return (
    <Dialog>
      <button onClick={() => useDialogContext()}>open</button>
    </Dialog>
  );
}

// Control: the hook runs in a component body under the dialog's own provider.
function Inside() {
  useDialogContext();
  return <span>inside</span>;
}

export function Controlled() {
  return (
    <Dialog>
      <Inside />
    </Dialog>
  );
}
