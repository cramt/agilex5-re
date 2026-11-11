"""Find the design sections of an Agilex 5 .rbf and rewrite their keyed hash
(+0xBB0), recomputing what a5db's section::assemble derives from the header:
the header CRC at +0xFFC, block 1's SHA-384 of the header and block 1's CRC.

  keyed.py list <rbf>
  keyed.py same <rbf> <out>            fix-ups only (must equal the input)
  keyed.py zero <rbf> <out> <index>...  zero these sections' keyed hashes (see list)
"""
import hashlib, struct, sys, zlib

PAGE = 0x1000

def crc32_bzip2(b):
    crc = 0xFFFFFFFF
    for byte in b:
        crc ^= byte << 24
        for _ in range(8):
            crc = ((crc << 1) ^ 0x04C11DB7) & 0xFFFFFFFF if crc & 0x80000000 else (crc << 1) & 0xFFFFFFFF
    return crc ^ 0xFFFFFFFF

def sections(d):
    out = []
    for at in range(0, len(d) - 2 * PAGE + 1, PAGE):
        h, b1 = d[at:at + PAGE], d[at + PAGE:at + 2 * PAGE]
        if struct.unpack_from('<I', h, 0xffc)[0] != crc32_bzip2(h[:0xffc]):
            continue
        if b1[:48] != hashlib.sha384(h).digest():
            continue
        size = struct.unpack_from('<I', h, 8)[0]
        out.append((at, size))
    return out

def kind(d, at):
    # Header bytes that differ between IO and CORE in a5db's tables; report raw.
    return d[at + 0xb1:at + 0xb7].hex()

def fix(d, at):
    h = bytearray(d[at:at + PAGE])
    struct.pack_into('<I', h, 0xffc, crc32_bzip2(bytes(h[:0xffc])))
    b1 = bytearray(d[at + PAGE:at + 2 * PAGE])
    b1[:48] = hashlib.sha384(bytes(h)).digest()
    struct.pack_into('<I', b1, 0xffc, crc32_bzip2(bytes(b1[:0xffc])))
    d[at:at + PAGE] = h
    d[at + PAGE:at + 2 * PAGE] = b1

def main():
    cmd, src = sys.argv[1], sys.argv[2]
    d = bytearray(open(src, 'rb').read())
    secs = sections(d)
    if cmd == 'list':
        for i, (at, size) in enumerate(secs):
            print(f'{i} at 0x{at:06x} size {size} tag {kind(d, at)} keyed {d[at+0xbb0:at+0xbd0].hex()[:16]}..')
        return
    out = sys.argv[3]
    pick = {int(a) for a in sys.argv[4:]}
    assert cmd == 'same' or pick, 'zero needs section indices'
    for i, (at, _) in enumerate(secs):
        if cmd == 'zero' and i in pick:
            d[at + 0xbb0:at + 0xbd0] = bytes(32)
        fix(d, at)
    open(out, 'wb').write(d)

main()
