export default function later(run: () => void) {
  setTimeout(run, 0);
}
later(() => {});
