import { createContext, useContext } from "solid-js";

type Tabs = { active: () => string; orientation: () => string };
const TabsContext = createContext<Tabs>();
const useTabs = () => useContext(TabsContext);

export { TabsContext, useTabs };
