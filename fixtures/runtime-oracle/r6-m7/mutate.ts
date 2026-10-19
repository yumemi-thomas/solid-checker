import { createSignal } from "solid-js";
import { api } from "./api";
const [count] = createSignal(0);
api.read = () => count();
