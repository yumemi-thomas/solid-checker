export function mark(id: string): void {
  const go = () => {
    const el = document.getElementById("ran-" + id);
    if (el) el.textContent = "ran";
    else setTimeout(go, 10);
  };
  go();
}
