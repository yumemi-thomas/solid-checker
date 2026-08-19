import {
  Reader,
  equivalentReader,
  genericRead,
  handlerTable,
  invoke,
  objectReader,
  quietReader,
} from "./source";

export function ClassCall() {
  const value = new Reader().read("class");
  return <div>{value}</div>;
}

export function GenericCall() {
  const value = invoke(objectReader, "generic");
  return <div>{value}</div>;
}

export function AmbiguousCall() {
  const value = invoke(
    Math.random() > 0.5 ? objectReader : quietReader,
    "ambiguous",
  );
  return <div>{value}</div>;
}

export function EquivalentConditionalCall() {
  const value = invoke(
    Math.random() > 0.5 ? objectReader : equivalentReader,
    "equivalent",
  );
  return <div>{value}</div>;
}

export function ComputedCall() {
  const value = handlerTable[Math.random() > 0.5 ? 0 : 1]();
  return <div>{value}</div>;
}

export function GenericFunctionCall() {
  const value = genericRead<string>("generic");
  return <div>{value}</div>;
}

export function ObjectCall() {
  const value = objectReader.read("object");
  return <div>{value}</div>;
}

export function ValueHelper() {
  const read = () => 42;
  return <div>{read()}</div>;
}

export function Shadowed() {
  const objectReader = { read: () => 42 };
  return <div>{objectReader.read("shadow")}</div>;
}
