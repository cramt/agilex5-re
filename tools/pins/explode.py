# a5db idle-diff output -> uid \t bit \t value lines (optionally only uids matching a prefix)
import sys
pre = tuple(sys.argv[2:]) or ('',)
for l in open(sys.argv[1]):
    f = l.rstrip('\n').split('\t')
    if len(f) < 3 or not f[0].startswith(pre): continue
    for kv in f[2].split(' '):
        if not kv: continue
        if f[0] == 'unnamed':
            print(f[0], kv, sep='\t'); continue
        b, v = kv.rsplit('=', 1)
        print(f[0], b, v, sep='\t')
