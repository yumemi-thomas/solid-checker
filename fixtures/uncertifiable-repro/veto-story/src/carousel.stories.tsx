// kui kernel VirtualizedTable.stories.tsx:316,320: a story `play` function dispatches on an element it found with
// querySelector; the receiver is unknown, so no handler of the project is placed (ADR 0356).
import { Carousel } from "./carousel";

export const Paged = {
  render: () => <Carousel pages={3} />,
  play: ({ canvasElement }: { canvasElement: HTMLElement }) => {
    const scroller = canvasElement.querySelector("button");
    scroller?.dispatchEvent(new Event("scroll"));
  },
};
