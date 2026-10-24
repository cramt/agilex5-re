# Turn a Quartus Pro PDB_ASCII_DUMP trace (<file>.ddb.dmp) into the token
# stream that `ddb trace <file.ddb>` prints, for a byte-exact comparison.
#
#   @CLASS          a deferred object body begins
#   H<w> <hex>      a traced integer of w bytes, as Quartus printed it
#   P <raw> <addr>  a pointer: raw serialized value, in-memory address
#   S <text>        string bytes
#   D <text>        a traced double (Quartus prints %.15E)
#
# String lengths are printed twice by Quartus (n, then n+1 for the C string
# it builds); only n is stored, so the second line is dropped.
#
# usage: python3 -I trace_tokens.py file.ddb.dmp > trace.tok
import re
import sys

hexre = re.compile(r'^ {4}([0-9A-F]+) +(.*)$')
ptrre = re.compile(r'^ {3}@([0-9a-f]+) +([+-]\d+) (.*)$')
strre = re.compile(r'^ {4}= "(.*)"$', re.S)
objre = re.compile(r'^@[0-9a-f]+: (.*)$')
boolre = re.compile(r'^ {5,}([01]) +(\S.*)$')
dblre = re.compile(r'^ {4}(-?\d\.\d+E[+-]\d+|-?nan|-?inf) +(.*)$', re.I)


def main(path):
    out = sys.stdout
    with open(path, 'rb') as f:
        lines = (l.decode('latin1').rstrip('\n') for l in f)
        pending = None  # a hex line held back to see whether it is a string length
        in_body = False
        for l in lines:
            if not in_body:
                if 'HEADER:' in l or 'FOOTER:' in l or l.startswith('    = "'):
                    continue
                in_body = True
            m = hexre.match(l)
            if m:
                name = m.group(2).strip()
                if pending is not None:
                    pv, pw, pname = pending
                    if pname == name and int(m.group(1), 16) == pv + 1 and len(m.group(1)) == 8:
                        # second length line of a string: keep the first only
                        out.write(f'H{pw} {pv:0{2 * pw}X}\n')
                        pending = None
                        continue
                    out.write(f'H{pw} {pv:0{2 * pw}X}\n')
                    pending = None
                h = m.group(1)
                if len(h) == 1:
                    # bools print as a bare 0/1
                    out.write(f'H1 {int(h):02X}\n')
                    continue
                if len(h) == 8:
                    pending = (int(h, 16), 4, name)
                else:
                    out.write(f'H{len(h) // 2} {h}\n')
                continue
            if pending is not None:
                pv, pw, _ = pending
                out.write(f'H{pw} {pv:0{2 * pw}X}\n')
                pending = None
            if not l.strip():
                continue
            m = objre.match(l)
            if m:
                out.write(f'@{m.group(1)}\n')
                continue
            m = ptrre.match(l)
            if m:
                out.write(f'P {m.group(2)} {m.group(1)}\n')
                continue
            m = strre.match(l)
            if m:
                out.write(f'S {m.group(1)}\n')
                continue
            m = boolre.match(l)
            if m:
                out.write(f'H1 {int(m.group(1)):02X}\n')
                continue
            m = dblre.match(l)
            if m:
                out.write(f'D {m.group(1)}\n')
                continue
            out.write(f'? {l}\n')
        if pending is not None:
            pv, pw, _ = pending
            out.write(f'H{pw} {pv:0{2 * pw}X}\n')


if __name__ == '__main__':
    main(sys.argv[1])
