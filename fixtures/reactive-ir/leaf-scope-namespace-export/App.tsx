import { onSettled } from "solid-js";
import * as utils from "./helpers";
import * as barrel from "./barrel";
import * as mutable from "./mutable";

export function NamespaceForbidden() {
  onSettled(() => { utils.forbidden(); });
  return <div />;
}
export function NamespaceConstForbidden() {
  onSettled(() => { utils.constant(); });
  return <div />;
}
export function NamespaceClean() {
  onSettled(() => { utils.pure(); });
  return <div />;
}
export function ReexportForbidden() {
  onSettled(() => { barrel.renamed(); });
  return <div />;
}
export function WrappedNamespaceForbidden() {
  onSettled(() => {
    ((utils satisfies typeof utils).forbidden satisfies () => void)();
  });
  return <div />;
}
export function ShadowedNamespace(props: { utils: { pure(): number } }) {
  const utils = props.utils;
  onSettled(() => { utils.pure(); });
  return <div />;
}
export function ComputedMember() {
  const key = "forbidden";
  onSettled(() => { utils[key](); });
  return <div />;
}
export function AliasedNamespace() {
  const alias = utils;
  onSettled(() => { alias.pure(); });
  return <div />;
}
export function MutableExports() {
  onSettled(() => {
    mutable.mutable();
    mutable.reassigned();
    mutable.wrapped();
  });
  return <div />;
}
export function UnspecializedCallback() {
  onSettled(() => { utils.supplied(() => {}); });
  return <div />;
}
export function DefaultParameterStillOpen() {
  onSettled(() => { utils.pureWithDefault(); });
  return <div />;
}
