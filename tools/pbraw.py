# Schema-less protobuf decoder: nested messages become lists of (field, value),
# printed one leaf per line with a dotted path, so files can be grepped and
# diffed.  python3 -I tools/pbraw.py file.pb [skip-bytes]
import sys


def varint(b, p):
    v = s = 0
    while True:
        c = b[p]; p += 1
        v |= (c & 0x7f) << s; s += 7
        if c < 0x80:
            return v, p


def parse(b):
    """Return [(field, value)] or None when b is not a clean message."""
    out, p = [], 0
    try:
        while p < len(b):
            k, p = varint(b, p)
            f, wt = k >> 3, k & 7
            if f == 0:
                return None
            if wt == 0:
                v, p = varint(b, p)
            elif wt == 1:
                v = int.from_bytes(b[p:p + 8], 'little'); p += 8
            elif wt == 5:
                v = int.from_bytes(b[p:p + 4], 'little'); p += 4
            elif wt == 2:
                n, p = varint(b, p)
                if p + n > len(b):
                    return None
                v = b[p:p + n]; p += n
            else:
                return None
            out.append((f, v))
    except IndexError:
        return None
    return out if p == len(b) else None


def walk(b, path, emit):
    for f, v in parse(b) or []:
        q = f'{path}.{f}' if path else str(f)
        if isinstance(v, bytes):
            if v and all(32 <= c < 127 for c in v):
                emit(q, repr(v.decode()))
            elif v and parse(v) is not None:
                emit(q, '{')
                walk(v, q, emit)
            else:
                emit(q, 'bytes:' + v[:32].hex() + ('…' if len(v) > 32 else ''))
        else:
            emit(q, v)


if __name__ == '__main__':
    data = open(sys.argv[1], 'rb').read()[int(sys.argv[2], 0) if len(sys.argv) > 2 else 0:]
    walk(data, '', lambda q, v: print(q, v))
