import { CalledDirectly, CalledOnAny, TemplateSubstitution, Unwritten } from "./App";

// The page's entry: the product's own modules, and no package that writes a prototype.
export const product = [CalledDirectly, CalledOnAny, TemplateSubstitution, Unwritten];

// A load by a specifier nothing proves: any module may be the one.
export const load = (name: string) => import(name);
