// error-menu-web AddProjectModal.tsx:97-122: setters called by the onOpenChange handler given to a Kobalte Dialog, and by a reset helper only that handler calls.
import { Dialog } from "@kobalte/core/dialog";
import { createSignal } from "solid-js";

export const AddProjectModal = () => {
  const [isOpen, setIsOpen] = createSignal(false);
  const [remoteUrl, setRemoteUrl] = createSignal("");
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);

  const resetForm = (): void => {
    setRemoteUrl("");
    setErrorMessage(null);
  };

  const handleOpenChange = (isNowOpen: boolean): void => {
    setIsOpen(isNowOpen);

    if (!isNowOpen) resetForm();
  };

  return (
    <Dialog open={isOpen()} onOpenChange={handleOpenChange}>
      <Dialog.Trigger>{errorMessage() ?? "New project"}</Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Content>
          <Dialog.Title>{remoteUrl()}</Dialog.Title>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog>
  );
};
