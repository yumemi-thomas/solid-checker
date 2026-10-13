import { createStore } from "solid-js";

// A write to the store root's own property, outside any setter. The runtime
// drops it on every Solid 2 release this checker has read (the rc.9 review's
// probe H, dev and prod). Whether it is this checker's to report depends on
// the published `Store<T>` -- see README.md.
export function rootWriteOutsideSetter() {
  const [profile] = createStore({ name: "Ada", user: { name: "Ada" } });
  profile.name = "Grace";
  // A nested record is writable to TypeScript on every release, and dropped
  // just the same, so this one is SC2003 on both twins.
  profile.user.name = "Grace";
  return profile;
}

// Inside the store's own setter the original proxy is write-enabled, so both
// writes commit: never a finding, on either twin.
export function rootWriteInsideOwnSetter() {
  const [profile, setProfile] = createStore({ name: "Ada" });
  setProfile(() => {
    profile.name = "Grace";
  });
  setProfile((draft) => {
    draft.name = "Ida";
  });
  return profile;
}
