# Board-test variants of an .rbf (README, "Board test"): change one header
# field of the design sections and redo everything that covers it (keyed
# hash, block CRCs, block 1's SHA-384), or corrupt one data byte and leave
# the hashes stale as a negative control.
#   python3 -I tools/rbf_patch.py zero-io-hash     in.rbf out.rbf
#   python3 -I tools/rbf_patch.py zero-design-hash in.rbf out.rbf
#   python3 -I tools/rbf_patch.py corrupt-core     in.rbf out.rbf
import hashlib
import hmac
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bitstream  # noqa: E402

# (section, offset) of the IO hash and the design hash (README (g2)).
IO_HASH = {'IO': 0x348, 'CORE': 0x234}
DESIGN_HASH = {'CORE': 0x1E0}


def sections(d):
    return {t: (base, size) for t, base, size in bitstream.sections(d)}


def resign(d, base):
    key = open(bitstream.KEY_FILE, 'rb').read()
    d[base + 0xBB0:base + 0xBD0] = hmac.new(key, bytes(d[base:base + 0xBB0]), hashlib.sha256).digest()
    d[base + 0xFFC:base + 0x1000] = struct.pack('<I', bitstream.crc32_bzip2(bytes(d[base:base + 0xFFC])))
    b1 = base + 0x1000
    d[b1:b1 + 48] = hashlib.sha384(bytes(d[base:base + 0x1000])).digest()
    d[b1 + 0xFFC:b1 + 0x1000] = struct.pack('<I', bitstream.crc32_bzip2(bytes(d[b1:b1 + 0xFFC])))


def main(cmd, inp, out):
    d = bytearray(open(inp, 'rb').read())
    secs = sections(d)
    if cmd == 'corrupt-core':
        base, size = secs['CORE']
        d[base + size - 0x5000] ^= 0x01
    else:
        fields = {'zero-io-hash': IO_HASH, 'zero-design-hash': DESIGN_HASH}[cmd]
        for name, at in fields.items():
            base, _ = secs[name]
            d[base + at:base + at + 32] = bytes(32)
            resign(d, base)
    open(out, 'wb').write(d)


if __name__ == '__main__':
    main(*sys.argv[1:])
