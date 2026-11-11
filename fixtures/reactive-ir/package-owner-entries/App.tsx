import { useTicker, Ticker, ui } from "dep-owner";

export function Called() {
  return <button onClick={() => useTicker()}>x</button>;
}
export function Dotted() {
  return <button onClick={() => useTicker.call(null)}>x</button>;
}
export function Tagged() {
  return <button onClick={() => useTicker`x`}>x</button>;
}
export function Constructed() {
  return <button onClick={() => new Ticker()}>x</button>;
}
export function Getter() {
  return <button onClick={() => ui.ticker}>x</button>;
}
