import { Reader, genericRead, invoke, objectReader, quietReader } from "./source";

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
