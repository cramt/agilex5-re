# Compare `ddb trace` output with trace_tokens.py output. Pointer tokens are
# compared on the raw value; the decoder's table#id and the trace's address
# must also be a bijection (same object <-> same address).
# usage: python3 -I tokcmp.py rust.tok trace.tok
import sys
a = open(sys.argv[1], encoding='latin1'); b = open(sys.argv[2], encoding='latin1')
ida = {}; adi = {}; n = 0; bad = 0
for n, (x, y) in enumerate(zip(a, b), 1):
    x = x.rstrip('\n'); y = y.rstrip('\n')
    if x.startswith('P ') and y.startswith('P '):
        xs = x.split(' '); ys = y.split(' ')
        if xs[1] != ys[1]:
            print(f'{n}: {x!r} != {y!r}'); bad += 1
        elif len(xs) > 2:
            obj, addr = xs[2], ys[2]
            if ida.setdefault(obj, addr) != addr or adi.setdefault(addr, obj) != obj:
                print(f'{n}: pointer {obj} -> {addr}, but seen {ida[obj]} / {adi[addr]}'); bad += 1
    elif x != y:
        print(f'{n}: {x!r} != {y!r}'); bad += 1
    if bad >= 10:
        break
rest_a = a.readline(); rest_b = b.readline()
if bad == 0 and (rest_a or rest_b):
    print(f'length differs after {n} tokens: rust {rest_a!r} trace {rest_b!r}'); bad += 1
print(f'{n} tokens compared, {len(ida)} objects pointed to, {"OK" if bad == 0 else "MISMATCH"}')
sys.exit(1 if bad else 0)
