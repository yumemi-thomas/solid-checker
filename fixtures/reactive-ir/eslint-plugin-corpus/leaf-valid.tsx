import {
  createContext,
  createSignal,
  createTrackedEffect,
  onSettled,
} from "solid-js";

createTrackedEffect(() => {
  const buildLater = () => createSignal(0);
  void buildLater;
});

createTrackedEffect(() => {
  const [value] = createSignal(0);
  void value;
});

onSettled(() => {
  const context = createContext("light");
  void context;
});
