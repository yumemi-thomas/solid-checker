// TypeScript accepts every value in this file, but a union may hide whether
// Solid receives a function or a bound pair. That case is explicitly
// `uncertifiable`; assertions are inspected at their runtime value.

type Handlers = [(data: number, event: MouseEvent) => void, number];
declare const pairOrFunction: Handlers | ((event: MouseEvent) => void);

const handler = (event: MouseEvent) => console.log(event);

export function UncertainHandlers() {
  return (
    <div>
      <button onClick={pairOrFunction} />
      {/* The assertion cannot change the array Solid receives: violation. */}
      <button
        onClick={([handler, 1] as unknown as (event: MouseEvent) => void)}
      />
      {/* The same escape around a real function is proven safe. */}
      <button
        onClick={(handler as unknown as (event: MouseEvent) => void)}
      />
    </div>
  );
}
