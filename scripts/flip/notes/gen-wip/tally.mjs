const fs = await import("node:fs");
const L = fs.readFileSync(process.argv[2], "utf8").trim().split("\n").map(JSON.parse);
const want = process.argv[3] ? new Set(JSON.parse(fs.readFileSync(process.argv[3], "utf8")).ids) : null;
const c = {};
for (const r of L) { if (want && !want.has(r.id)) continue; const k = `${r.outcome} chrome=${r.chrome?.status ?? "-"}${r.chrome?.cache ? " cache=" + r.chrome.cache : ""}`; c[k] = (c[k] || 0) + 1; }
console.log(L.length, want ? `(of ${want.size} wanted)` : "", c);
if (process.argv[4]) for (const r of L) if (!want || want.has(r.id)) console.log(r.id, r.outcome, JSON.stringify(r.detail?.findings));
