# Print the container structure of an Agilex 5 .sof or .rbf, or check an
# .rbf's block CRCs and keyed hashes.
#   python3 -I tools/bitstream.py design.sof|design.rbf
#   python3 -I tools/bitstream.py key            # once: derive the HMAC key
#   python3 -I tools/bitstream.py check design.rbf
# .sof: gzip of "SOF\0", u32 0, u32 record count, then (u16 tag, u32 len, data).
# .rbf: 4 KiB-aligned SDM sections, each with header magic 0x97566593 (at
#       +0x400 in the first, CMF, section; at +0 in the others), u32 size at
#       +8 and a 4-character type at +0xC (Security TRM, "SDM Firmware
#       Section Header Block").
import gzip
import hashlib
import hmac
import os
import re
import struct
import subprocess
import sys
import zlib

SECTION_MAGIC = 0x97566593
RBF_MAGIC = 0x62294895
BITASM_LIB = '/pool/dev/altera_pro/quartus-pro/26.1.1/quartus/linux64/libpgm_bitasm.so'
KEY_FILE = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'specimens', 'rbf_keyed_hash.key')
# The keyed hash of a design section: HMAC-SHA256 over [section, +0xBB0),
# stored at +0xBB0 (BITASM_COMMON::calculate_keyed_hash / verify_keyed_hash).
KEYED_HASH_AT = 0xBB0


def derive_key(lib=BITASM_LIB):
    """calculate_keyed_hash builds its 32-byte key from two rip-relative
    16-byte loads; find them in its disassembly."""
    nm = subprocess.run(['nm', '-D', lib], capture_output=True, text=True).stdout
    fn = int(re.search(r'^([0-9a-f]+) T _ZN13BITASM_COMMON20calculate_keyed_hashEPKhj$', nm, re.M).group(1), 16)
    dis = subprocess.run(['objdump', '-d', f'--start-address={fn}', f'--stop-address={fn + 0x40}', lib],
                         capture_output=True, text=True).stdout
    addrs = [int(a, 16) for a in re.findall(r'movdqa 0x[0-9a-f]+\(%rip\),%xmm0\s+# ([0-9a-f]+)', dis)]
    data = open(lib, 'rb').read()
    return b''.join(data[a:a + 16] for a in addrs[:2])


def crc32_bzip2(b):
    # MSB-first CRC-32 (poly 0x04C11DB7, init/xorout 0xFFFFFFFF) from the
    # reflected one: bit-reverse input bytes and the result.
    rev = bytes(int(f'{i:08b}'[::-1], 2) for i in range(256))
    c = zlib.crc32(b.translate(rev))
    return int(f'{c:032b}'[::-1], 2)


def check(d):
    """Recompute the per-block CRCs and the design sections' keyed hashes."""
    key = open(KEY_FILE, 'rb').read()
    u = lambda o: struct.unpack_from('<I', d, o)[0]
    base, ok = 0, True
    while base < len(d):
        h = base + (0x400 if base == 0 else 0)
        size, typ = u(h + 8), d[h + 12:h + 16].rstrip(b'\0').decode()
        for blk in (base, base + 0x1000):
            good = struct.unpack_from('<I', d, blk + 0xFFC)[0] == crc32_bzip2(d[blk:blk + 0xFFC])
            ok &= good
            print(f'  {typ:5s} block {blk:#08x} CRC {"ok" if good else "BAD"}')
        if typ != 'CMF':
            want = hmac.new(key, d[base:base + KEYED_HASH_AT], hashlib.sha256).digest()
            good = d[base + KEYED_HASH_AT:base + KEYED_HASH_AT + 32] == want
            ok &= good
            print(f'  {typ:5s} keyed hash {"ok" if good else "BAD"}')
            ok &= section_hashes(d[base:base + size], typ)
        base += size
    return ok


def section_hashes(S, typ):
    """The section hash (+0x3C, SHA-256 prefix of [+0x2000, end)), its
    SHA-384 at +0xBD0, the first chain table's at +0xC0, block 1's SHA-384
    of block 0, and every chain entry against its page (README (l))."""
    body = S[0x2000:]
    checks = {
        'section hash': S[0x3C:0x40] == hashlib.sha256(body).digest()[:4],
        'body SHA-384': S[0xBD0:0xC00] == hashlib.sha384(body).digest(),
        'chain table SHA-384': S[0xC0:0xF0] == hashlib.sha384(S[0x2000:0x3000]).digest(),
        'header SHA-384': S[0x1000:0x1030] == hashlib.sha384(S[:0x1000]).digest(),
    }
    # Chain entries in order, each matched to the next page with that hash.
    k, todo, ents, bad = 3, [0x2000], [], 0
    while todo:
        p = todo.pop(0)
        n = struct.unpack_from('<I', S, p + 8)[0]
        ents += [(S[p + 16 + 52 * i:p + 64 + 52 * i], struct.unpack_from('<I', S, p + 64 + 52 * i)[0]) for i in range(n)]
        while ents:
            h, f = ents.pop(0)
            while k * 4096 < len(S) and hashlib.sha384(S[k * 4096:(k + 1) * 4096]).digest() != h:
                k += 1
            if k * 4096 >= len(S):
                bad += 1
                break
            if f == 4:
                todo.append(k * 4096)
                k += 1
                break
            k += 1
        if bad:
            break
    checks['chain'] = bad == 0 and not ents
    for name, good in checks.items():
        print(f'  {typ:5s} {name} {"ok" if good else "BAD"}')
    return all(checks.values())


def sof(raw):
    d = gzip.decompress(raw) if raw[:2] == b'\x1f\x8b' else raw
    assert d[:4] == b'SOF\0', 'not a SOF'
    n, = struct.unpack_from('<I', d, 8)
    print(f'SOF, {len(d)} bytes uncompressed, {n} records')
    p = 12
    while p < len(d):
        tag, ln = struct.unpack_from('<HI', d, p)
        body = d[p + 6:p + 6 + ln]
        text = body[:40].split(b'\0')[0]
        show = text.decode('latin1') if text.isascii() and len(text) > 3 else body[:16].hex()
        print(f'  tag {tag:#06x} at {p:#09x} len {ln:9d}  {show}')
        p += 6 + ln


def rbf(d):
    u = lambda o: struct.unpack_from('<I', d, o)[0]
    assert u(0) == RBF_MAGIC, 'not an RBF'
    print(f'RBF, {len(d)} bytes')
    base = 0
    while base < len(d):
        h = base + (0x400 if base == 0 else 0)
        assert u(h) == SECTION_MAGIC, f'no section magic at {h:#x}'
        size, typ = u(h + 8), d[h + 12:h + 16].rstrip(b'\0').decode()
        ver = u(h + 0x20)
        extra = ''
        if typ == 'CMF':
            build = d[0xfe0:0xffc].split(b'\0')[0].decode()
            extra = f' firmware {ver >> 24}.{ver >> 16 & 0xff}.{ver >> 8 & 0xff}, build id {build!r}'
        sha = hashlib.sha256(d[base:base + size]).hexdigest()[:16]
        print(f'  {typ:5s} at {base:#08x} size {size:#08x} ({size} bytes){extra}  sha256 {sha}...')
        base += size


if __name__ == '__main__':
    if sys.argv[1] == 'key':
        os.makedirs(os.path.dirname(KEY_FILE), exist_ok=True)
        open(KEY_FILE, 'wb').write(derive_key(*sys.argv[2:3]))
    elif sys.argv[1] == 'check':
        sys.exit(0 if check(open(sys.argv[2], 'rb').read()) else 1)
    else:
        raw = open(sys.argv[1], 'rb').read()
        rbf(raw) if raw[:4] == struct.pack('<I', RBF_MAGIC) else sof(raw)
