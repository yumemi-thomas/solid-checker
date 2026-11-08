#!/usr/bin/env python3 -I
# sweepdiff.py <base.json> <new.json>: rows added/removed by full key (rule kind path start end blocker), V moves,
# per-app U, removed rows with no row left on their line.
import json, sys, collections
def rows(p):
    d = json.load(open(p)); out = collections.Counter()
    for r in d['results']:
        for f in r.get('findings', []):
            loc = f['primaryLocation']
            out[(r['app'], r['project'], f['rule'], f['kind'], loc['path'], loc.get('startByte'), loc.get('endByte'), loc.get('line'), f.get('analysisContext', '').split(':')[0])] += 1
    return out, d['findingKinds']
b, bk = rows(sys.argv[1]); n, nk = rows(sys.argv[2])
print('kinds base', bk, 'new', nk)
rem = b - n; add = n - b
print('removed', sum(rem.values()), 'added', sum(add.values()))
print('V removed', [k for k in rem.elements() if k[3] == 'violation'])
print('V added', [k for k in add.elements() if k[3] == 'violation'])
newl = collections.Counter((k[0], k[1], k[4], k[7]) for k in n.elements())
gone = [k for k in rem.elements() if newl[(k[0], k[1], k[4], k[7])] == 0]
print('removed with no row left on the line:', len(gone))
for k in gone[:30]: print('  ', k)
print('removed by blocker', dict(collections.Counter(k[8] for k in rem.elements())))
print('added by blocker', dict(collections.Counter(k[8] for k in add.elements())))
print('added by rule', dict(collections.Counter(k[2] for k in add.elements())))
print('added by app', dict(sorted(collections.Counter(k[0] for k in add.elements()).items(), key=lambda x: -x[1])))
print('removed by app', dict(collections.Counter(k[0] for k in rem.elements())))
