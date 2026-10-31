# Decode the CORE section of an Agilex 5 (sm4revb) .rbf back to fabric CRAM.
#
# quartus_pfg stores the two `sectorbnxplv` CRAM sectors of the .sof image
# (4704 frames x 540 bytes each) with "2D compression". README (g),
# "CORE section: 2D compression", describes the format; in short:
#
#   pages     CORE is 4 KiB pages. The SHA-384 chain tables (+0x2000, then
#             every page whose entry flag is 4) list the pages in order with
#             a stream id: 0 = sector 0, 1 = sector 1.
#   per stream, in page order: lib pages (count from the 0x15 kind-4 action
#             at +0x6000), a codebook page, a deflated 2D-lib page, a second
#             codebook page, the BBM page, then the raw-deflate frame stream.
#   BBM       15 x (id, width/16 bits, 98, 0), a u16, then 48 rows of
#             (u16, 60 nibbles): row 0 = IP id per column, rows 1..47 = one
#             code per column for each 98-frame group.
#   frames    per frame `04 00 a4 00`, 4 zero bytes, then the literal IP
#             columns (codes 0 and >= 8) in column order. Packets run over
#             frame groups 0, 2, 3, ..., 47; group 1 (frames 98..195) is
#             never stored.
#   codes     0 literal, 1 zero, 2..7 copy of lib (code-2), 8/9 literal XOR
#             (frame - 18) on the class-1/2 words (frames 26..97 of a group),
#             10+k literal XOR lib k.
#   physical  each frame is a 656-byte physical frame; layout() maps it to
#             the 4320-bit dr (S4 groups from sof_meta.pb become 4 copied
#             bits and two 4-bit nibbles).
#
#   python3 -I tools/core_decode.py decode in.rbf out.bin       # 2 x 4704 x 540 bytes
#   python3 -I tools/core_decode.py verify in.rbf in.sof        # compare with the .sof image
#   python3 -I tools/core_decode.py reencode in.rbf             # rebuild the packet streams
import hashlib
import os
import struct
import sys
import zlib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pbraw  # noqa: E402

CORE_OFF = 0xcf000
NF, FB, PF = 4704, 540, 656        # frames per sector, dr bytes, physical bytes
GROUP = 98
STORED_GROUPS = [0] + list(range(2, 48))
HDR = b'\x04\x00\xa4\x00\0\0\0\0'
SOF_META = os.environ.get('SOF_META', '/pool/dev/altera_pro/quartus-pro/26.1.1/devices/10nm/sm4revb/sof_meta.pb')

# dr bits with no physical bit: pfg drops them [obs, rand specimen]
DROPPED = [(256, 275), (1664, 1675), (2080, 2099), (2496, 2515), (3392, 3403), (3808, 3827), (4192, 4215)]
# 4-bit dr groups stored as 2 pad bits, (b+2, b+3), 2 pad bits, (b, b+1)
PADDED_PAIRS = (4264, 3488, 1760)
PAD32_AT = 2528                    # 32 always-zero physical bits
ROW_HISTORY = 18                   # row prediction looks 18 frames back ...
ROW_FIRST = 26                     # ... for frames 26..97 of a group
# byte offsets inside an IP column that codes 8 (class 1) and 9 (class 2) XOR
ROW_MASK = {(1, 8): (8, 9), (1, 9): (6, 7), (2, 8): (0, 1), (2, 9): (2, 3),
            (3, 8): (10, 11), (3, 9): (12, 13), (4, 8): (4, 5), (4, 9): (6, 7)}


def s4_positions(path=SOF_META):
    """Bit offsets of the 's4' groups of sectorbnxplv_dr (field 4.2 of sof_meta.pb)."""
    for f, v in pbraw.parse(open(path, 'rb').read()):
        if f != 4:
            continue
        for f2, v2 in pbraw.parse(v):
            if f2 != 2:
                continue
            fields = pbraw.parse(v2)
            if (1, b'sectorbnxplv_dr') in fields:
                return [dict(pbraw.parse(e))[2] for g, e in fields if g == 3]
    raise SystemExit('sectorbnxplv_dr not found in ' + path)


def layout(s4):
    """Physical bit -> ('c', dr bit) | ('n', a, b) at the nibble's first bit | None."""
    drop = {d for a, b in DROPPED for d in range(a, b + 1)}
    gs, out, d = set(s4), [], 4319
    while d >= 0:
        if d in drop:
            d -= 1
        elif d - 3 in PADDED_PAIRS:
            b = d - 3
            out += [None, None, ('c', b + 2), ('c', b + 3), None, None, ('c', b), ('c', b + 1)]
            d = b - 1
        elif d - 7 in gs:
            g = d - 7
            out += [('c', g + 5), ('c', g + 1), ('c', g + 4), ('c', g),
                    ('n', g + 3, g + 7), None, None, None, ('n', g + 2, g + 6), None, None, None]
            d = g - 1
        else:
            out.append(('c', d))
            d -= 1
    out = out[:PAD32_AT] + [None] * 32 + out[PAD32_AT:]
    assert len(out) == PF * 8
    return out


class Layout:
    def __init__(self, s4):
        self.map = layout(s4)
        self.copy = [(pb, v[1]) for pb, v in enumerate(self.map) if v and v[0] == 'c']
        self.nibs = [(pb, v[1], v[2]) for pb, v in enumerate(self.map) if v and v[0] == 'n']
        self.dropped = {d for a, b in DROPPED for d in range(a, b + 1)}

    def to_dr(self, ph):
        """Physical frame -> 540 dr bytes. Returns (dr, flagged S4 nibbles)."""
        bits = ''.join(_BITS[c] for c in ph)
        dr = [0] * (FB * 8)
        for pb, d in self.copy:
            if bits[pb] == '1':
                dr[d] = 1
        flagged = 0
        for pb, a, b in self.nibs:
            n = bits[pb:pb + 4]
            if n[0] == '1':
                # The flag bit means exactly one of a, b is set, and which one
                # depends on a condition outside the frame [unk]; (1, 0) is
                # by far the commoner and nibbles 1100/1110 only occur for (0, 1).
                flagged += 1
                dr[a], dr[b] = (0, 1) if n in ('1100', '1110') else (1, 0)
            else:
                dr[a], dr[b] = int(n[2]), int(n[3])
        return bytes(int(''.join(map(str, dr[i + 7:i - 1 if i else None:-1])), 2) for i in range(0, FB * 8, 8)), flagged

    def to_phys(self, dr):
        """dr -> physical frame, S4 pairs always as the plain 00ab nibble."""
        bits = ''.join(_BITS[c] for c in dr)
        out = ['0'] * (PF * 8)
        for pb, d in self.copy:
            out[pb] = bits[d]
        for pb, a, b in self.nibs:
            out[pb + 2], out[pb + 3] = bits[a], bits[b]
        s = ''.join(out)
        return bytes(int(s[i:i + 8][::-1], 2) for i in range(0, len(s), 8))


_BITS = [format(i, '08b')[::-1] for i in range(256)]   # LSB first


def tlvs(C, base):
    out, p = [], base
    while True:
        t, ln = struct.unpack_from('<II', C, p)
        if t == 0 or ln < 8 or ln > 0x1000:
            return out
        out.append((t, list(struct.unpack_from('<%dI' % ((ln - 8) // 4), C, p + 8))))
        p += ln


def page_streams(C):
    """Stream id of each 4 KiB page, from the SHA-384 chain tables in page order."""
    def entries(p):
        n = struct.unpack_from('<I', C, p + 8)[0]
        return [(C[p + 16 + 52 * i:p + 64 + 52 * i], struct.unpack_from('<I', C, p + 64 + 52 * i)[0]) for i in range(n)]
    fl = [None] * (len(C) // 4096)
    ents, i = entries(0x2000), 0
    for k in range(3, len(fl)):
        if i == len(ents):
            break
        h, f = ents[i]
        if hashlib.sha384(C[k * 4096:(k + 1) * 4096]).digest() == h:
            fl[k] = f
            i += 1
            if f == 4:
                ents += entries(k * 4096)
    return fl


def streams(C):
    fl = page_streams(C)
    acts = tlvs(C, 0x6000)
    nlib = {v[1]: v[0] for t, v in acts if t == 0x15 and v[2] == 4}
    setup = {v[0]: v for t, v in acts if t == 0x1f}
    out = {}
    for s in (0, 1):
        pages = [C[k * 4096:(k + 1) * 4096] for k in range(len(fl)) if fl[k] == s]
        n = nlib[s]
        o = zlib.decompressobj(-15)
        frames = o.decompress(b''.join(pages[n + 4:]))
        assert o.eof, 'frame stream does not end'
        out[s] = dict(lib=b''.join(pages[:n]), bbm=pages[n + 3], frames=frames, setup=setup[s],
                      raw=b''.join(pages[n + 4:]))
    return out


def bbm(T):
    width = {T[i]: T[i + 1] * 2 for i in range(0, 60, 4)}     # bytes
    rows = []
    for k in range(48):
        r = T[62 + 32 * k:94 + 32 * k]
        rows.append([(r[2 + j // 2] >> (4 * (j & 1))) & 15 for j in range(60)])
    return width, rows


def libs_of(st, ids, W):
    """{ip id: [lib][frame in group] -> bytes} from the 0x1f setup record's 8-byte offsets."""
    w = st['setup']
    offs = [0, w[1] >> 16, w[2] & 0xffff, w[2] >> 16, w[3] & 0xffff]
    libs = {}
    for t in (1, 2, 3, 4):
        wid = W[ids.index(t)]
        stride = (wid + 3) // 4 * 4 if wid < 12 else (wid + 7) // 8 * 8
        a, b = offs[t - 1] * 8, offs[t] * 8
        libs[t] = [[st['lib'][a + (L * GROUP + i) * stride:][:wid] for i in range(GROUP)]
                   for L in range((b - a) // (GROUP * stride))]
    return libs


def decode_stream(st):
    """Physical frames {frame index: 656 bytes} of one sector."""
    width, rows = bbm(st['bbm'])
    ids = rows[0]
    W = [width[i] for i in ids]
    start = [sum(W[:j]) for j in range(60)]
    libs = libs_of(st, ids, W)
    x, p, out = st['frames'], 0, {}
    for r, g in enumerate(STORED_GROUPS, 1):
        code = rows[r]
        lit = [j for j in range(60) if code[j] == 0 or code[j] >= 8]
        fr = []
        for i in range(GROUP):
            assert x[p:p + 8] == HDR, f'bad packet header at {p}'
            p += 8
            ph = bytearray(PF)
            for j in lit:
                ph[start[j]:start[j] + W[j]] = x[p:p + W[j]]
                p += W[j]
            fr.append(ph)
        for j in range(60):
            c, t, a, e = code[j], ids[j], start[j], start[j] + W[j]
            for i in range(GROUP):
                if 2 <= c <= 7:
                    fr[i][a:e] = libs[t][c - 2][i]
                elif c >= 10:
                    fr[i][a:e] = bytes(u ^ v for u, v in zip(fr[i][a:e], libs[t][c - 10][i]))
                elif c in (8, 9) and i >= ROW_FIRST:
                    for k in ROW_MASK[(t, c)]:
                        fr[i][a + k] ^= fr[i - ROW_HISTORY][a + k]
        for i in range(GROUP):
            out[GROUP * g + i] = bytes(fr[i])
    assert p == len(x), 'trailing frame-stream bytes'
    return out


def encode_stream(st, phys):
    """Inverse of decode_stream with the same BBM and libs: the inflated packet stream."""
    width, rows = bbm(st['bbm'])
    ids = rows[0]
    W = [width[i] for i in ids]
    start = [sum(W[:j]) for j in range(60)]
    libs = libs_of(st, ids, W)
    out = bytearray()
    for r, g in enumerate(STORED_GROUPS, 1):
        code = rows[r]
        for i in range(GROUP):
            ph = phys[GROUP * g + i]
            out += HDR
            for j in range(60):
                c, t, a, e = code[j], ids[j], start[j], start[j] + W[j]
                if c == 0:
                    out += ph[a:e]
                elif c >= 10:
                    out += bytes(u ^ v for u, v in zip(ph[a:e], libs[t][c - 10][i]))
                elif c in (8, 9):
                    col = bytearray(ph[a:e])
                    if i >= ROW_FIRST:
                        prev = phys[GROUP * g + i - ROW_HISTORY]
                        for k in ROW_MASK[(t, c)]:
                            col[k] ^= prev[a + k]
                    out += col
    return bytes(out)


def decode(rbf, lay):
    st = streams(rbf[CORE_OFF:])
    return st, {s: decode_stream(st[s]) for s in (0, 1)}


def main():
    cmd = sys.argv[1]
    rbf = open(sys.argv[2], 'rb').read()
    lay = Layout(s4_positions())
    st, phys = decode(rbf, lay)
    if cmd == 'reencode':
        for s in (0, 1):
            same = encode_stream(st[s], phys[s]) == st[s]['frames']
            print(f'sector {s}: packet stream {"identical" if same else "DIFFERS"}')
        return
    sectors = {}
    flagged = 0
    for s in (0, 1):
        img = bytearray(NF * FB)
        for f, ph in phys[s].items():
            dr, n = lay.to_dr(ph)
            flagged += n
            img[f * FB:(f + 1) * FB] = dr
        sectors[s] = bytes(img)
    if cmd == 'decode':
        open(sys.argv[3], 'wb').write(sectors[0] + sectors[1])
        print(f'flagged S4 nibbles: {flagged}')
    elif cmd == 'verify':
        import sof
        meta = sof.records(sof.load(sys.argv[3]))  # noqa: F841 (checks the magic)
        image = sof.image(sof.load(sys.argv[3]))
        keep = bytearray(b'\xff' * FB)
        for d in lay.dropped:
            keep[d >> 3] &= ~(1 << (d & 7)) & 0xff
        bad = 0
        for s, base in ((0, 13691080), (1, 16723044)):
            want = image[base:base + NF * FB]
            for g in STORED_GROUPS:
                for f in range(GROUP * g, GROUP * (g + 1)):
                    a, b = sectors[s][f * FB:(f + 1) * FB], want[f * FB:(f + 1) * FB]
                    n = sum(bin((u ^ v) & k).count('1') for u, v, k in zip(a, b, keep))
                    if n:
                        bad += 1
                        if bad <= 10:
                            print(f'sector {s} frame {f}: {n} bits differ')
        print(f'{bad} frames differ (group 1 and the {len(lay.dropped)} dropped dr bits excluded); '
              f'{flagged} flagged S4 nibbles')
        sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
