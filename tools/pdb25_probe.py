# Probe a Quartus Pro PDB v25 .ddb: header fields, then decompress body and footer.
# Usage: python3 -I tools/pdb25_probe.py <file.ddb>...
import struct, sys, zlib, lzma, re
def parse(path):
    b = open(path, 'rb').read()
    ver, magic, _u6, _u8, _u10, hsz, hend = struct.unpack_from('<iHHHHHH', b, 0)
    slen, = struct.unpack_from('<i', b, 0x10)
    s = b[0x14:0x14+slen-1].decode()
    o = 0x14 + slen - 1
    body_sz, body_end, foot_sz = struct.unpack_from('<QQQ', b, o); o += 24
    rest = b[o:hend]
    return b, dict(ver=ver, magic=hex(magic), hsz=hsz, hend=hend, creator=s,
                   body_sz=body_sz, body_end=body_end, foot_sz=foot_sz, extra=rest.hex(), size=len(b))
def decomp(chunk, want):
    if chunk[:2] == b'\x78\x01' or chunk[:1] == b'\x78':
        d = zlib.decompressobj(); return 'zlib', d.decompress(chunk)
    if chunk[0] == 0x5d:
        dict_size, = struct.unpack_from('<I', chunk, 1)
        f = [{'id': lzma.FILTER_LZMA1, 'dict_size': dict_size, 'lc': 3, 'lp': 0, 'pb': 2}]
        d = lzma.LZMADecompressor(format=lzma.FORMAT_RAW, filters=f)
        return 'lzma1-raw', d.decompress(chunk[5:], max_length=want)
    return 'unknown', b''
for p in sys.argv[1:]:
    b, h = parse(p)
    print(p.split('/')[-1], {k: v for k, v in h.items() if k != 'extra'})
    print('  hdr extra:', h['extra'])
    kind, body = decomp(b[h['hend']:h['body_end']], h['body_sz'])
    print('  body', kind, len(body), 'of', h['body_sz'])
    foot_raw = b[h['body_end']:]
    kind2, foot = decomp(foot_raw, h['foot_sz'])
    print('  footer', kind2, len(foot), 'of', h['foot_sz'])
    names = sorted(set(re.findall(rb'[A-Z][A-Z0-9_:]{5,}', foot)))[:12]
    print('  footer names:', [n.decode() for n in names])
