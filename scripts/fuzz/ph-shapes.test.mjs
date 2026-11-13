import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { PH_CODES, PH_CONTEXTS, PH_CONTEXT_NAMES, PH_HOSTS, PH_PACKAGE, PH_ROUTES, phPackageFiles, phParts, phShape } from "./ph-shapes.mjs";

// The B3 PR after #213 and its flip (the review of #216, and the review of #217's causes A to E): a flush in a function a package the build does not read as source may run, by
// every route a function reaches the package (forty-seven), with the twins that never run it.

const built = (code, context) => {
  const side = PH_CONTEXTS[context];
  const shape = (side === "misuse" ? recallShapes : recallLegalShapes)(code, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: side === "legal" } });
  const case_ = buildCase(program, { id: `ph-${context}` });
  expect(isValid(program), context).toBe(true);
  expect(case_.neutral, context).toBe(true);
  expect(case_.sites, context).toHaveLength(1);
  return case_;
};
const textOf = (code, context) => built(code, context).text.split("\n").map((line) => line.trim());

describe("the ph contexts (a flush in a function a package nothing here reads may run)", () => {
  it("are seventy-six routes on three hosts (dynref, spreadref, spreadvar, childfn and the frame routes on the step host only), each with a twin that never runs the site, each name parsed back", () => {
    expect(PH_ROUTES).toEqual(["literal", "ref", "alias", "member", "method", "holder", "array", "jsx", "children", "wrap", "caller", "return", "call", "set", "forof", "recv", "resolve", "class", "assign", "props", "getter", "reflect", "inline", "param", "untrack", "dynamic", "mapinit", "mapdestr", "mapparam", "mapflat", "maptemplate", "maplogical", "mapawait", "mapassign", "maparg", "yieldof", "nbmap", "nbobj", "argsindex", "argsapply", "thenreturn", "thenconst", "thenlet", "thenfn", "thenmember", "refprop", "dynref", "chainconst", "chainassign", "chainrecv", "spreadref", "spreadvar", "childfn", "framechild", "framebare", "framemap", "framecall", "framerender", "frameiife", "framemapref", "framevoid", "frametypeof", "propsread", "propsrender", "getterparam", "selfgetter", "treerec", "propsjsx", "propsclass", "getterthis", "getterouter", "getterassign", "propsmethod", "getterpick", "gettersuper", "methodsuper"]);
    expect(PH_HOSTS).toEqual(["step", "apply", "leaf"]);
    expect(PH_CONTEXT_NAMES).toHaveLength(404);
    for (const name of PH_CONTEXT_NAMES) {
      const parts = phParts(name);
      expect(parts, name).not.toBeNull();
      expect(PH_CONTEXTS[name]).toBe(parts.never ? "legal" : "misuse");
      expect(name).toBe(`ph-${parts.route}-${parts.host}${parts.never ? "-never" : ""}`);
    }
    for (const other of ["ph-literal", "fc-named-action", "ph-literal-other", "pl-untrack-tracked"]) expect(phParts(other), other).toBeNull();
  });
  it("ship a package that names no Solid package, so the build does not read its source", () => {
    const files = phPackageFiles();
    const manifest = JSON.parse(files["package.json"]);
    expect(manifest.name).toBe(PH_PACKAGE);
    expect(manifest.dependencies).toBeUndefined();
    expect(manifest.peerDependencies).toBeUndefined();
    expect(files["index.js"]).toContain("action(function* ()");
    expect(files["index.js"]).toContain("createEffect(() => 1, () => {");
    expect(files["index.js"]).toContain("createTrackedEffect(() => {");
    for (const name of ["runStep", "runApplyWith", "runLeafAll", "runStepGet", "runApplyConfig", "StepView", "ApplyView", "LeafView", "skipRun", "skipRunWith", "skipRunAll", "skipRunGet", "skipRunConfig", "SkipView"]) expect(files["index.d.ts"], name).toContain(`export declare const ${name}:`);
    expect(files["index.d.ts"]).toContain("export declare const config:");
    const program = built("FLUSH_IN_ACTION", "ph-literal-step");
    expect(Object.keys(program.packages ?? {})).toEqual([PH_PACKAGE]);
  });
  it("print the routes: the site first, then the call that hands the function to the package, or in the twin to a function that ignores it", () => {
    const literal = textOf("FLUSH_IN_ACTION", "ph-literal-step");
    expect(literal).toContain('import { runStep } from "fz-step";');
    expect(literal.some((line) => line.startsWith("runStep(() => {"))).toBe(true);
    const twin = textOf("FLUSH_IN_ACTION", "ph-literal-step-never");
    expect(twin).toContain('import { skipRun } from "fz-step";');
    expect(twin.some((line) => line.startsWith("skipRun(() => {"))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-ref-step").some((line) => /^runStep\(cb\d+\);$/.test(line))).toBe(true);
    const alias = textOf("FLUSH_IN_EFFECT_CALLBACK", "ph-alias-apply");
    expect(alias.some((line) => /^const again\d+ = cb\d+;$/.test(line))).toBe(true);
    expect(alias.some((line) => /^runApply\(again\d+\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-member-step").some((line) => line.startsWith("runStepWith({ run: () => {"))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-method-step").some((line) => line.startsWith("runStepWith({ run() {"))).toBe(true);
    const holder = textOf("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "ph-holder-leaf");
    expect(holder.some((line) => line.startsWith("const opts"))).toBe(true);
    expect(holder.some((line) => /^runLeafWith\(opts\d+\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-array-step").some((line) => line.startsWith("runStepAll([() => {"))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-jsx-step").some((line) => line.startsWith("document.body.append((<StepView render={() => {"))).toBe(true);
    expect(textOf("FLUSH_IN_EFFECT_CALLBACK", "ph-children-apply").some((line) => line.startsWith("document.body.append((<ApplyView>{() => {"))).toBe(true);
    expect(textOf("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "ph-jsx-leaf-never").some((line) => line.includes("<SkipView render={() => {"))).toBe(true);
    const wrap = textOf("FLUSH_IN_ACTION", "ph-wrap-step");
    expect(wrap.some((line) => /^function wrap\d+\(run: \(\) => void\) \{$/.test(line))).toBe(true);
    expect(wrap).toContain("runStep(run);");
    const returned = textOf("FLUSH_IN_ACTION", "ph-return-step");
    expect(returned.some((line) => /^function make\d+\(\) \{$/.test(line))).toBe(true);
    expect(returned.some((line) => /^runStep\(make\d+\(\)\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-call-step").some((line) => line.startsWith("runStep.call(null, () => {"))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-call-step-never").some((line) => line.startsWith("skipRun.call(null, () => {"))).toBe(true);
    const caller = textOf("FLUSH_IN_ACTION", "ph-caller-step");
    expect(caller.some((line) => /^function helper\d+\(\) \{$/.test(line))).toBe(true);
    expect(caller.some((line) => /^runStep\(\(\) => helper\d+\(\)\);$/.test(line))).toBe(true);
  });
  it("print the flip's routes: where the value is kept, iterated, returned, assigned, run inline, or passed along a component", () => {
    const set = textOf("FLUSH_IN_ACTION", "ph-set-step");
    expect(set.some((line) => /^const kept\d+ = new Set<\(\) => void>\(\);$/.test(line))).toBe(true);
    expect(set.some((line) => /^kept\d+\.add\(\(\) => \{$/.test(line))).toBe(true);
    expect(set.some((line) => /^runStep\(each\d+\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-forof-step").some((line) => /^for \(const each\d+ of \[\(\) => \{$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-recv-step").some((line) => /^\[\(\) => \{$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-resolve-step").some((line) => line.startsWith("void Promise.resolve(() => {"))).toBe(true);
    const klass = textOf("FLUSH_IN_ACTION", "ph-class-step");
    expect(klass.some((line) => /^class Job\d+ \{$/.test(line))).toBe(true);
    expect(klass.some((line) => /^runStepWith\(new Job\d+\(\)\);$/.test(line))).toBe(true);
    const assign = textOf("FLUSH_IN_EFFECT_CALLBACK", "ph-assign-apply");
    expect(assign).toContain('import { runApplyConfig, config } from "fz-step";');
    expect(assign.some((line) => line.startsWith("config.onStep = () => {"))).toBe(true);
    expect(assign).toContain("runApplyConfig();");
    expect(textOf("FLUSH_IN_ACTION", "ph-assign-step-never")).toContain("skipRunConfig();");
    const props = textOf("FLUSH_IN_ACTION", "ph-props-step");
    expect(props.some((line) => /^function Wrap\d+\(props: \{ render: \(\) => void \}\) \{$/.test(line))).toBe(true);
    expect(props).toContain("return <StepView render={props.render} />;");
    expect(textOf("FLUSH_IN_ACTION", "ph-getter-step").some((line) => /^runStepGet\(\(\) => cb\d+\);$/.test(line))).toBe(true);
    const reflect = textOf("FLUSH_IN_ACTION", "ph-reflect-step");
    expect(reflect.some((line) => /^wrap\d+\.call\(null, \(\) => \{$/.test(line))).toBe(true);
    expect(reflect).toContain("runStep(run);");
    expect(textOf("FLUSH_IN_ACTION", "ph-inline-step").some((line) => /^\[1\]\.forEach\(cb\d+\);$/.test(line))).toBe(true);
    const param = textOf("FLUSH_IN_ACTION", "ph-param-step");
    expect(param.some((line) => /^function call\d+\(g: \(\) => void\) \{$/.test(line))).toBe(true);
    expect(param.some((line) => /^call\d+\(cb\d+\);$/.test(line))).toBe(true);
    const untrack = textOf("FLUSH_IN_ACTION", "ph-untrack-step");
    expect(untrack).toContain('import { untrack } from "solid-js";');
    expect(untrack.some((line) => /^untrack\(cb\d+\);$/.test(line))).toBe(true);
    const dynamic = textOf("FLUSH_IN_ACTION", "ph-dynamic-step");
    expect(dynamic).toContain('import { Dynamic } from "@solidjs/web";');
    expect(dynamic.some((line) => line.startsWith("document.body.append((<Dynamic component={StepView} render={() => {"))).toBe(true);
  });
  it("print the review of #217's routes: a literal inside a receiver in a context, `arguments`, a returned function's call, an intrinsic ref", () => {
    expect(textOf("FLUSH_IN_ACTION", "ph-mapinit-step").some((line) => /^const rs\d+ = \[cb\d+\]\.map\(\(each\) => runStep\(each\)\);$/.test(line))).toBe(true);
    const destr = textOf("FLUSH_IN_ACTION", "ph-mapdestr-step");
    expect(destr.some((line) => /^const \[first\d+\] = fs\d+;$/.test(line))).toBe(true);
    expect(destr.some((line) => /^const rs\d+ = \[first\d+\]\.map\(\(each\) => runStep\(each\)\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-mapparam-step").some((line) => /^const rs\d+ = \[g\]\.map\(\(each\) => runStep\(each\)\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-mapflat-step").some((line) => /\[\[cb\d+\]\]\.flat\(\)\.map\(/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-maptemplate-step").some((line) => /^const text\d+ = `\$\{\[cb\d+\]\.map\(\(each\) => runStep\(each\)\)\.length\}`;$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-maplogical-step").some((line) => /^const ok\d+ = \{ a: cb\d+ \}\.a\.name\.length > 0 && \[cb\d+\]\.some\(\(each\) => \{$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-mapawait-step").some((line) => /^await \[cb\d+\]\.map\(\(each\) => runStep\(each\)\)\[0\];$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-mapassign-step").some((line) => /^kept\d+ = \[cb\d+\]\.map\(\(each\) => runStep\(each\)\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-maparg-step").some((line) => /^use\d+\(\[cb\d+\]\.map\(\(each\) => runStep\(each\)\)\);$/.test(line))).toBe(true);
    const gen = textOf("FLUSH_IN_ACTION", "ph-yieldof-step");
    expect(gen.some((line) => /^const x\d+ = yield cb\d+;$/.test(line))).toBe(true);
    expect(gen.some((line) => /^for \(const each of gen\d+\(\)\) \{$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-nbmap-step").some((line) => /^const rs\d+ = \[cb\d+\]\.map\(\(each\) => each\(\)\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-nbobj-step").some((line) => /^const r\d+ = \{ run: cb\d+ \}\.run\(\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-argsindex-step")).toContain("runStep(arguments[0]);");
    const apply = textOf("FLUSH_IN_ACTION", "ph-argsapply-step");
    expect(apply.some((line) => /^rest\d+\.apply\(null, arguments as unknown as Array<\(\) => void>\);$/.test(line))).toBe(true);
    const then = textOf("FLUSH_IN_ACTION", "ph-thenreturn-step");
    expect(then.some((line) => /^\.then\(\(\) => cb\d+\)$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-thenreturn-step-never")).toContain("skipRun(each);");
    expect(textOf("FLUSH_IN_ACTION", "ph-refprop-step").some((line) => /^void \(<div ref=\{cb\d+\} \/>\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-dynref-step").some((line) => /^void \(<Dynamic component="div" ref=\{cb\d+\} \/>\);$/.test(line))).toBe(true);
  });
  it("print the review of #222's routes: an assignment expression's own value, an intrinsic element's spread and function child, a component under a package's component", () => {
    const chain = textOf("FLUSH_IN_ACTION", "ph-chainconst-step");
    expect(chain.some((line) => /^const first\d+ = \(last\d+ = cb\d+\);$/.test(line))).toBe(true);
    expect(chain.some((line) => /^runStep\(first\d+\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_EFFECT_CALLBACK", "ph-chainassign-apply").some((line) => /^first\d+ = last\d+ = cb\d+;$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "ph-chainrecv-leaf").some((line) => /^const rs\d+ = \[\(last\d+ = cb\d+\)\]\.map\(\(each\) => runLeaf\w*\(each\)\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-spreadref-step").some((line) => /^void \(<div \{\.\.\.\{ ref: cb\d+ \}\} \/>\);$/.test(line))).toBe(true);
    const spreadvar = textOf("FLUSH_IN_ACTION", "ph-spreadvar-step");
    expect(spreadvar.some((line) => /^const props\d+ = \{ ref: cb\d+ \};$/.test(line))).toBe(true);
    expect(spreadvar.some((line) => /^void \(<div \{\.\.\.props\d+\} \/>\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-childfn-step").some((line) => /^void \(<div>\{cb\d+ as never\}<\/div>\);$/.test(line))).toBe(true);
    const frame = textOf("FLUSH_IN_ACTION", "ph-framechild-step");
    expect(frame.some((line) => /^const Own\d+ = \(\(_props: \{ children\?: unknown \}\) => \{$/.test(line))).toBe(true);
    expect(frame.some((line) => /^document\.body\.append\(\(<StepView>\{\(<Own\d+>x<\/Own\d+>\) as never\}<\/StepView>\) as unknown as Node\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-framebare-step-never").some((line) => /^document\.body\.append\(\(<SkipView>\{\(<Own\d+ \/>\) as never\}<\/SkipView>\) as unknown as Node\);$/.test(line))).toBe(true);
    const at = (context, pattern) => expect(textOf("FLUSH_IN_ACTION", context).some((line) => pattern.test(line)), context).toBe(true);
    at("ph-framemap-step", /^document\.body\.append\(\(<StepView>\{\[1\]\.map\(\(\) => <Own\d+ \/>\) as never\}<\/StepView>\) as unknown as Node\);$/);
    at("ph-framecall-step", /^document\.body\.append\(\(<StepView>\{helper\d+\(\) as never\}<\/StepView>\) as unknown as Node\);$/);
    at("ph-framerender-step", /^document\.body\.append\(\(<StepView render=\{make\d+\(\) as never\} \/>\) as unknown as Node\);$/);
    at("ph-frameiife-step", /^document\.body\.append\(\(<StepView>\{\(\(\) => <Own\d+ \/>\)\(\) as never\}<\/StepView>\) as unknown as Node\);$/);
    at("ph-framemapref-step", /^document\.body\.append\(\(<StepView>\{\[1\]\.map\(\(\) => <div ref=\{cb\d+\} \/>\) as never\}<\/StepView>\) as unknown as Node\);$/);
    at("ph-framevoid-step", /^document\.body\.append\(\(<StepView>\{void \(<Own\d+ \/>\)\}<\/StepView>\) as unknown as Node\);$/);
    at("ph-frametypeof-step-never", /^document\.body\.append\(\(<SkipView>\{typeof \(<Own\d+ \/>\) as never\}<\/SkipView>\) as unknown as Node\);$/);
    for (const route of ["spreadref", "spreadvar", "childfn", "framechild", "framebare", "framemap", "framecall", "framerender", "frameiife", "framemapref", "framevoid", "frametypeof"]) expect(PH_CONTEXT_NAMES.filter((name) => phParts(name).route === route)).toEqual([`ph-${route}-step`, `ph-${route}-step-never`]);
  });
  it("are cells of SPEC on their side for the codes their host reports, one base shape, not the census's, out of api-fire's plan", () => {
    expect(phShape("ph-literal-step", { binding: "alias", layer: "none" })).toBe(false);
    expect(phShape("pl-untrack-tracked", { binding: "alias", layer: "iife" })).toBe(true);
    for (const [code, hosts] of Object.entries(PH_CODES)) {
      for (const context of PH_CONTEXT_NAMES.filter((name) => hosts.includes(phParts(name).host))) {
        const side = PH_CONTEXTS[context] === "misuse" ? SPEC[code].recallContexts : SPEC[code].recallLegal;
        expect(side, `${code} ${context}`).toContain(context);
        const shapes = (PH_CONTEXTS[context] === "misuse" ? recallShapes : recallLegalShapes)(code, context);
        expect(shapes.length, `${code} ${context}`).toBeGreaterThan(0);
        for (const shape of shapes) expect({ binding: shape.binding, layer: shape.layer ?? "none" }, `${code} ${context}`).toEqual({ binding: "direct", layer: "none" });
      }
    }
    for (const context of PH_CONTEXT_NAMES) expect(BASE_ONLY_CONTEXTS()).toContain(context);
    expect(Object.keys(PH_CODES)).toEqual(["FLUSH_IN_EFFECT_CALLBACK", "FLUSH_IN_ACTION", "FLUSH_IN_TRACKED_QUEUE_CALLBACK"]);
    expect(censusCells().filter(({ context }) => PH_CONTEXT_NAMES.includes(context))).toEqual([]);
  });
  it("print the review of #222 check 3's routes: a prop read in a step or a render callback, a getter read through a parameter", () => {
    const read = textOf("FLUSH_IN_ACTION", "ph-propsread-step");
    expect(read.some((line) => /^function Wrap\d+\(props: \{ children\?: unknown \}\) \{$/.test(line))).toBe(true);
    expect(read).toContain("void props.children;");
    expect(read.some((line) => /^document\.body\.append\(\(<Wrap\d+>\{helper\d+\(\)\}<\/Wrap\d+>\) as unknown as Node\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_EFFECT_CALLBACK", "ph-propsrender-apply").some((line) => /^return <ApplyView render=\{\(\) => \{$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "ph-propsrender-leaf-never").some((line) => /^return <SkipView render=\{\(\) => \{$/.test(line))).toBe(true);
    const getter = textOf("FLUSH_IN_ACTION", "ph-getterparam-step");
    expect(getter).toContain("get g() {");
    expect(getter).toContain("void p.g;");
    expect(getter.some((line) => /^w\d+\(o\d+\);$/.test(line))).toBe(true);
  });
  it("print the review of #226's routes: a self-named getter, a self-rendering component, props read in JSX or a class field, a getter read through `this`", () => {
    expect(textOf("FLUSH_IN_ACTION", "ph-selfgetter-step").some((line) => /^return o\d+\.x;$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-treerec-step").some((line) => /^const kid = \(\) => <Tree\d+ depth=\{1\} pick=\{props\.pick\} x=\{helper\d+\(\)\} \/>;$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_EFFECT_CALLBACK", "ph-propsjsx-apply")).toContain("return <ApplyView>{typeof props.children as never}</ApplyView>;");
    expect(textOf("FLUSH_IN_ACTION", "ph-propsclass-step")).toContain("v = props.children === undefined;");
    expect(textOf("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "ph-getterthis-leaf")).toContain("void this.g;");
  });
  it("print the re-check of #226's routes: a method written outside the literal, one assigned later, a prop that is a method, a callback that returns the function or the object", () => {
    const outer = textOf("FLUSH_IN_ACTION", "ph-getterouter-step");
    expect(outer.some((line) => /^function m\d+\(this: \{ g: number \}\) \{$/.test(line))).toBe(true);
    expect(outer.some((line) => /^m: m\d+,$/.test(line))).toBe(true);
    expect(outer.some((line) => /^o\d+\.m\(\);$/.test(line))).toBe(true);
    const assigned = textOf("FLUSH_IN_EFFECT_CALLBACK", "ph-getterassign-apply");
    expect(assigned).toContain("m() {},");
    expect(assigned.some((line) => /^o\d+\.m = m\d+;$/.test(line))).toBe(true);
    const method = textOf("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "ph-propsmethod-leaf");
    expect(method).toContain("props.pick();");
    expect(method.some((line) => /^document\.body\.append\(\(<Wrap\d+ pick=\{m\d+\}>\{helper\d+\(\)\}<\/Wrap\d+>\) as unknown as Node\);$/.test(line))).toBe(true);
    const pick = textOf("FLUSH_IN_ACTION", "ph-getterpick-step");
    expect(pick.some((line) => /^const pick\d+ = \(\): any => \(flag\d+ \? helper\d+ : o\d+\);$/.test(line))).toBe(true);
    expect(pick.some((line) => /^const got\d+ = \[0\]\.map\(pick\d+\);$/.test(line))).toBe(true);
  });
  it("print the re-check 2 of #226's routes: an accessor that holds `super`, a method that holds it", () => {
    const getter = textOf("FLUSH_IN_ACTION", "ph-gettersuper-step");
    expect(getter).toContain("void (super.x as any);");
    expect(getter.some((line) => /^\(o\d+ as any\)\.__proto__ = base\d+;$/.test(line))).toBe(true);
    expect(getter.some((line) => /^void o\d+\.h;$/.test(line))).toBe(true);
    const method = textOf("FLUSH_IN_EFFECT_CALLBACK", "ph-methodsuper-apply");
    expect(method).toContain("runIt() {");
    expect(method.some((line) => /^o\d+\.runIt\(\);$/.test(line))).toBe(true);
  });
});
