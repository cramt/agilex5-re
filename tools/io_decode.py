# Decode the IO section of an Agilex 5 (sm4revb) .rbf back to the periphery
# parts of the .sof image, and rebuild the section's design streams from them.
#
# README (g), "IO section", describes the format; in short:
#
#   pages     IO is 4 KiB pages. The SHA-384 chain tables (+0x2000, then every
#             page whose entry flag is 4) list the hashed pages in order: flag
#             0x80000002 = firmware module headers, 2 = action pages, 0 = data.
#             SDM firmware modules (cssm.fw, cssm_ux_phy.fw, io_config.fw ...)
#             sit between them as unhashed pages, copied from the install.
#   streams   each run of flag-0 pages is one raw-deflate stream. In chain
#             order: perictrl0 data, perictrl2+1 headers, perictrl0 header,
#             io96 headers, io96 t41, io96 t40. pfg's deflate is zlib's
#             level 9, 8 KiB window, Z_FIXED: deflate() redoes it bit for bit.
#   commands  an inflated stream is 8-byte aligned commands. Low byte 0x0c or
#             0x0d: an 8-byte control word pair. Otherwise a send: u32 (target,
#             flags, u16 n), u32 0, n words, zero-padded to 8 bytes.
#   packets   [u32 total][u32 0x20][u32 0][u32 count][4 x u32 0], then count
#             records [u32 len][u32 index][len - 8 payload bytes].
#
#   python3 -I tools/io_decode.py layout in.rbf                 # pages and streams
#   python3 -I tools/io_decode.py decode in.rbf out.pkl         # {(object, part): (bytes, known mask)}
#   python3 -I tools/io_decode.py verify in.rbf in.sof          # decoded parts against the .sof image
#   python3 -I tools/io_decode.py encode in.sof ref.rbf [out.rbf]
#       rebuild the six inflated streams (and their deflate) from the .sof
#       image; ref.rbf supplies the perictrl0 register list, which is fixed
#       but not found in the install yet. Compares with out.rbf if given.
import hashlib
import os
import pickle
import struct
import sys
import zlib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), 'bitmap'))
from imagemap import PARTS  # noqa: E402

IO_OFF = 0x6a000
PART_LEN = {(o, p): n for o, p, s, n in PARTS}
_REV = bytes(int(f'{i:08b}'[::-1], 2) for i in range(256))

# stream roles in chain order
PD, PC21, PC0, IOHDR, T41, T40 = range(6)
# send targets of each stream, by target index
TARGETS = {PD: ['perictrl0'], PC21: ['perictrl2', 'perictrl1'], PC0: ['perictrl0'],
           IOHDR: ['io96_0', 'io96_1'], T41: ['io96_0', 'io96_1'], T40: ['io96_0', 'io96_1']}
CTL_PD = [(0x0400000d, 0), (0x3c00000c, 0)]     # after every perictrl0 data packet
CTL_IOHDR = [(0x00c8000c, 0)]                    # after every io96 header packet
MAX_N = 509                                      # a send is at most 2048 bytes


def deflate(x):
    o = zlib.compressobj(9, zlib.DEFLATED, -13, 8, zlib.Z_FIXED)
    return o.compress(x) + o.flush()


# ---- pages ---------------------------------------------------------------

def section(rbf):
    assert rbf[IO_OFF:IO_OFF + 4] == b'\x93\x65\x56\x97' and rbf[IO_OFF + 12:IO_OFF + 14] == b'IO', 'no IO section'
    return rbf[IO_OFF:IO_OFF + struct.unpack_from('<I', rbf, IO_OFF + 8)[0]]


def chain(C):
    """[(page, flag)] of the hashed pages, in chain order."""
    pages = {hashlib.sha384(C[k:k + 4096]).digest(): k // 4096 for k in range(0, len(C), 4096)}
    out, todo = [], [0x2000]
    while todo:
        p = todo.pop(0)
        for i in range(struct.unpack_from('<I', C, p + 8)[0]):
            k = pages[C[p + 16 + 52 * i:p + 64 + 52 * i]]
            f = struct.unpack_from('<I', C, p + 64 + 52 * i)[0]
            out.append((k, f))
            if f == 4:
                todo.append(k * 4096)
    return out


def raw_streams(C):
    """[(pages, deflated bytes, inflated bytes)] of the flag-0 page runs."""
    ch, out, i = chain(C), [], 0
    while i < len(ch):
        if ch[i][1] != 0:
            i += 1
            continue
        o, x, raw, pages = zlib.decompressobj(-15), b'', b'', []
        while not o.eof:
            k, f = ch[i]
            i += 1
            if f == 4:
                continue
            assert f == 0, 'stream interrupted'
            pg = C[k * 4096:(k + 1) * 4096]
            x += o.decompress(pg)
            raw += pg[:len(pg) - len(o.unused_data)] if o.eof else pg
            pages.append(k)
        out.append((pages, raw, x))
    assert len(out) == 6, f'{len(out)} design streams, expected 6'
    return out


# ---- commands and packets ------------------------------------------------

def commands(s):
    """[('ctl', w0, w1) | ('send', target, flags, body)]."""
    out, p = [], 0
    while p < len(s):
        w0, w1 = struct.unpack_from('<II', s, p)
        if w0 & 0xff in (0x0c, 0x0d):
            out.append(('ctl', w0, w1))
            p += 8
            continue
        n = w0 >> 16
        assert w1 == 0 and (w0 >> 8) & 0xfe == 0, f'unknown command {w0:08x} {w1:08x} at {p}'
        body = s[p + 8:p + 8 + 4 * n]
        pad = s[p + 8 + 4 * n:p + 8 + 4 * n + 4 * (n & 1)]
        assert len(body) == 4 * n and pad == bytes(len(pad)), f'bad send at {p}'
        out.append(('send', w0 & 0xff, (w0 >> 8) & 0xff, body))
        p += 8 + 4 * (n + (n & 1))
    return out


def send(target, flags, body):
    n = len(body) // 4
    assert len(body) == 4 * n and n < 0x10000
    return struct.pack('<II', target | flags << 8 | n << 16, 0) + body + bytes(4 * (n & 1))


def ctl(pairs):
    return b''.join(struct.pack('<II', a, b) for a, b in pairs)


def packet_records(pk):
    total, hl, z, count = struct.unpack_from('<4I', pk)
    assert total == len(pk) and hl == 0x20 and z == 0 and pk[16:32] == bytes(16), 'bad packet header'
    recs, p = [], 32
    for _ in range(count):
        ln, idx = struct.unpack_from('<II', pk, p)
        recs.append((idx, pk[p + 8:p + ln]))
        p += ln
    assert p == total
    return recs


def packet(records):
    """records: [(index, payload)]"""
    body = b''.join(struct.pack('<II', len(r) + 8, i) + r for i, r in records)
    return struct.pack('<8I', 32 + len(body), 0x20, 0, len(records), 0, 0, 0, 0) + body


# ---- perictrl headers: one record, the header bit-reversed ----------------
# payload = 8 zero bytes, bytes [0, len-24) of the header in reverse order
# with each byte bit-reversed, 2 zero bytes. The last 24 bytes are not sent.
PC_TAIL = 24


def pc_hdr_payload(hdr):
    return bytes(8) + hdr[:-PC_TAIL][::-1].translate(_REV) + bytes(2)


def pc_hdr_from_payload(pl, n):
    body = pl[8:8 + n - PC_TAIL]
    assert pl[:8] == bytes(8) and pl[8 + n - PC_TAIL:] == bytes(2)
    return body[::-1].translate(_REV) + bytes(PC_TAIL), b'\xff' * (n - PC_TAIL) + bytes(PC_TAIL)


def pc_hdr_sends(target, hdr):
    """A perictrl header packet, cut into sends of at most 2048 bytes; each
    starts with the packet offset it carries."""
    pk = packet([(0, pc_hdr_payload(hdr))])
    out, off = b'', 0
    while off < len(pk):
        part = pk[off:off + 4 * MAX_N]
        last = off + len(part) == len(pk)
        out += send(target, 1 if last else 0, struct.pack('<I', off) + part)
        off += len(part)
    return out


def pc_hdr_join(sends):
    """{target: packet bytes} from (target, flags, body) sends."""
    pk = {}
    for t, f, body in sends:
        off = struct.unpack_from('<I', body)[0]
        b = pk.setdefault(t, bytearray())
        assert off == len(b), 'perictrl header chunks out of order'
        b += body[4:]
    return pk


# ---- io96 headers: 10 blocks of bit fields --------------------------------
# block k = A (800 bits), B (1472), C, D; blocks start on a byte. Sent per
# kind (D, C, B, A), one packet per io96 with record k = block k's field,
# split into 32-bit words that go out last word first, after zero padding.
IO_C = [1472] * 8 + [744, 202]
IO_D = [1401, 1467, 1401, 1401, 1401, 1467, 1401, 1401, None, None]
IO_RLEN = {'D': 200, 'C': 200, 'B': 200, 'A': 116}


def io_fields():
    """{kind: [(start bit, bits) | None] x 10}"""
    f, pos = {k: [] for k in 'ABCD'}, 0
    for k in range(10):
        for kind, bits in (('A', 800), ('B', 1472), ('C', IO_C[k]), ('D', IO_D[k])):
            if bits is None:
                f[kind].append(None)
                continue
            f[kind].append((pos, bits))
            pos += bits
        pos = (pos + 7) // 8 * 8
    return f


IO_FIELDS = io_fields()


def io_field_payload(hdr, field, rlen):
    pl = bytearray(rlen - 8)
    if field is None:
        return bytes(pl)
    s, bits = field
    v = int.from_bytes(hdr[s // 8:(s + bits + 7) // 8], 'little') & ((1 << bits) - 1)
    nw = (bits + 31) // 32
    words = [(v >> (32 * j)) & 0xffffffff for j in range(nw)]
    pl[len(pl) - 4 * nw:] = struct.pack('<%dI' % nw, *words[::-1])
    return bytes(pl)


def io_hdr_stream(hdrs):
    out = b''
    for kind in 'DCBA':
        for t, hdr in enumerate(hdrs):
            recs = [(k, io_field_payload(hdr, IO_FIELDS[kind][k], IO_RLEN[kind])) for k in range(10)]
            out += send(t, 0, packet(recs)) + ctl(CTL_IOHDR)
    return out


def io_hdr_decode(cmds):
    n = PART_LEN[('io96_0', 'hdr')]
    vals, known = [0, 0], [0, 0]
    sends = [c for c in cmds if c[0] == 'send']
    assert len(sends) == 8
    for (kind, t), (_, tt, f, body) in zip([(k, t) for k in 'DCBA' for t in (0, 1)], sends):
        assert tt == t
        for k, pl in packet_records(body):
            field = IO_FIELDS[kind][k]
            if field is None:
                assert pl == bytes(len(pl))
                continue
            s, bits = field
            nw = (bits + 31) // 32
            words = struct.unpack_from('<%dI' % nw, pl, len(pl) - 4 * nw)[::-1]
            v = sum(w << (32 * j) for j, w in enumerate(words))
            assert v >> bits == 0, 'bits beyond an io96 header field'
            vals[t] |= v << s
            known[t] |= ((1 << bits) - 1) << s
    return [(vals[t].to_bytes(n, 'little'), known[t].to_bytes(n, 'little')) for t in (0, 1)]


# ---- io96 t40/t41: address + 2 KiB blocks ---------------------------------
# t41[0:1024] goes to address 0, t41[1024:] to 0x400000 + offset - 1024, but
# 0x47b000..0x47efff carry t40 instead and t41's last 4 KiB go to 0x47f000:
# t41[0x7b400:0x80400] is never sent.
T41_HEAD = 1024
T40_ADDR = 0x47b000


def t41_blocks():
    """[(address, t41 offset, length)] in send order."""
    out = [(0, 0, T41_HEAD)]
    for a in range(0x400000, 0x480000, 0x800):
        if T40_ADDR <= a < 0x47f000:
            continue
        o = T41_HEAD + a - 0x400000 + (0x1000 if a >= 0x47f000 else 0)
        out.append((a, o, 0x800))
    return out


def t40_blocks():
    return [(T40_ADDR + o, o, 0x800) for o in range(0, PART_LEN[('io96_0', 't40')], 0x800)]


def mem_stream(blocks, datas):
    out = b''
    for i, (a, o, ln) in enumerate(blocks):
        for t, d in enumerate(datas):
            out += send(t, 1 if i == len(blocks) - 1 else 0, struct.pack('<I', a) + d[o:o + ln])
    return out


def mem_decode(cmds, blocks, n):
    vals = [bytearray(n), bytearray(n)]
    known = [bytearray(n), bytearray(n)]
    where = {a: (o, ln) for a, o, ln in blocks}
    sends = [c for c in cmds if c[0] == 'send']
    assert len(sends) == 2 * len(blocks)
    for _, t, f, body in sends:
        o, ln = where[struct.unpack_from('<I', body)[0]]
        assert len(body) == 4 + ln
        vals[t][o:o + ln] = body[4:]
        known[t][o:o + ln] = b'\xff' * ln
    return [(bytes(vals[t]), bytes(known[t])) for t in (0, 1)]


# ---- perictrl0 data: register write list ----------------------------------
# packet = [u32 1][u32 4n][records (byte address, byte length, data)]; a
# record is cut to fill a packet, and a packet closes when < 12 bytes remain.

def pd_records(cmds):
    recs = []
    for c in cmds:
        if c[0] == 'ctl':
            continue
        _, t, f, body = c
        one, ln = struct.unpack_from('<II', body)
        assert t == 0 and one == 1 and ln == len(body)
        p = 8
        while p < len(body):
            a, l = struct.unpack_from('<II', body, p)
            recs.append((a, body[p + 8:p + 8 + l]))
            p += 8 + l
        assert p == len(body)
    return recs


def pd_ranges(recs):
    out = []
    for a, d in recs:
        if out and out[-1][0] + out[-1][1] == a:
            out[-1][1] += len(d)
        else:
            out.append([a, len(d)])
    return out


def pd_stream(data, ranges):
    cap = 4 * MAX_N - 8
    out, cur = b'', b''

    def close():
        nonlocal out, cur
        out += send(0, 0, struct.pack('<II', 1, 8 + len(cur)) + cur) + ctl(CTL_PD)
        cur = b''
    for a, l in ranges:
        while l:
            if cap - len(cur) < 12:
                close()
            take = min(l, (cap - len(cur) - 8) // 4 * 4)
            cur += struct.pack('<II', a, take) + data[a:a + take]
            a, l = a + take, l - take
    close()
    return out


# ---- whole section ---------------------------------------------------------

def decode_streams(xs):
    """{(object, part): (bytes, known mask)} from the six inflated streams."""
    out = {}
    cmds = [commands(x) for x in xs]
    n = PART_LEN[('perictrl0', 'data')]
    d, k = bytearray(n), bytearray(n)
    for a, b in pd_records(cmds[PD]):
        d[a:a + len(b)] = b
        k[a:a + len(b)] = b'\xff' * len(b)
    out[('perictrl0', 'data')] = (bytes(d), bytes(k))
    for si in (PC21, PC0):
        sends = [c[1:] for c in cmds[si] if c[0] == 'send']
        assert len(sends) == len(cmds[si]), 'control words in a perictrl header stream'
        for t, pk in pc_hdr_join(sends).items():
            obj = TARGETS[si][t]
            (idx, pl), = packet_records(bytes(pk))
            out[(obj, 'hdr')] = pc_hdr_from_payload(pl, PART_LEN[(obj, 'hdr')])
    for t, v in enumerate(io_hdr_decode(cmds[IOHDR])):
        out[(f'io96_{t}', 'hdr')] = v
    for si, part, blocks in ((T41, 't41', t41_blocks()), (T40, 't40', t40_blocks())):
        for t, v in enumerate(mem_decode(cmds[si], blocks, PART_LEN[('io96_0', part)])):
            out[(f'io96_{t}', part)] = v
    return out


def encode_streams(parts, ranges):
    """The six inflated streams from {(object, part): bytes}."""
    return [
        pd_stream(parts[('perictrl0', 'data')], ranges),
        pc_hdr_sends(0, parts[('perictrl2', 'hdr')]) + pc_hdr_sends(1, parts[('perictrl1', 'hdr')]),
        pc_hdr_sends(0, parts[('perictrl0', 'hdr')]),
        io_hdr_stream([parts[('io96_0', 'hdr')], parts[('io96_1', 'hdr')]]),
        mem_stream(t41_blocks(), [parts[('io96_0', 't41')], parts[('io96_1', 't41')]]),
        mem_stream(t40_blocks(), [parts[('io96_0', 't40')], parts[('io96_1', 't40')]]),
    ]


def sof_parts(path):
    import sof
    im = sof.image(sof.load(path))
    return {(o, p): im[s:s + n] for o, p, s, n in PARTS}


NAMES = ['perictrl0 data', 'perictrl2+1 hdr', 'perictrl0 hdr', 'io96 hdr', 'io96 t41', 'io96 t40']


def main():
    cmd = sys.argv[1]
    if cmd == 'encode':
        parts = sof_parts(sys.argv[2])
        ranges = pd_ranges(pd_records(commands(raw_streams(section(open(sys.argv[3], 'rb').read()))[PD][2])))
        xs = encode_streams(parts, ranges)
        if len(sys.argv) > 4:
            ref = raw_streams(section(open(sys.argv[4], 'rb').read()))
            bad = 0
            for name, x, (pages, raw, y) in zip(NAMES, xs, ref):
                same, dsame = x == y, deflate(x) == raw
                bad += not (same and dsame)
                print(f'{name:16} {len(x):8} bytes  inflated {"identical" if same else "DIFFERS"}  '
                      f'deflated {"identical" if dsame else "DIFFERS"}')
            sys.exit(1 if bad else 0)
        for name, x in zip(NAMES, xs):
            print(f'{name:16} {len(x):8} bytes, {len(deflate(x))} deflated')
        return
    C = section(open(sys.argv[2], 'rb').read())
    rs = raw_streams(C)
    if cmd == 'layout':
        role = {}
        for i, (pages, raw, x) in enumerate(rs):
            for k in pages:
                role[k] = f'stream {i} ({NAMES[i]})'
        hashed = dict(chain(C))
        for k in range(len(C) // 4096):
            what = role.get(k) or {None: 'unhashed (firmware module page)' if k > 2 else 'section header',
                                   0x80000002: 'firmware module header', 2: 'action list', 4: 'chain table'}.get(
                hashed.get(k), f'flag {hashed.get(k)}')
            print(f'+0x{k * 4096:05x} {what}')
        for i, (pages, raw, x) in enumerate(rs):
            print(f'stream {i} {NAMES[i]:16} pages {len(pages):3}  deflated {len(raw):7}  inflated {len(x):8}')
        return
    dec = decode_streams([x for _, _, x in rs])
    if cmd == 'decode':
        pickle.dump(dec, open(sys.argv[3], 'wb'))
        for k, (v, m) in sorted(dec.items()):
            print(f'{k[0]:10} {k[1]:5} {sum(bin(c).count("1") for c in m):8} of {8 * len(m)} bits known')
    elif cmd == 'verify':
        parts, bad = sof_parts(sys.argv[3]), 0
        for k, (v, m) in sorted(dec.items()):
            want = parts[k]
            diff = sum(1 for a, b, c in zip(v, want, m) if (a ^ b) & c)
            unsent = sum(1 for b, c in zip(want, m) if b & ~c)
            bad += diff
            sent = sum(bin(c).count('1') for c in m)
            print(f'{k[0]:10} {k[1]:5} {sent:8}/{8 * len(m)} bits sent, {diff} bytes differ; '
                  f'{unsent} bytes with set bits the IO section does not carry')
        sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
