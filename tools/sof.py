# Read and write Agilex 5 .sof files.
#
# A .sof is gzip of "SOF\0", u32 0, u32 record count, then (u16 tag, u32 len,
# data) records. The last record (tag 0x08) holds a CRC-16/X-25 (reflected
# poly 0x8408, init and xorout 0xFFFF), little endian, over every byte before
# its own data, its tag and length included (PGM_COMMON::calculate_sof_crc16
# in libpgm_common.so; libpgm_bitasm's BITASM_SOF_HELPER feeds it).
# Before it, record 0x41 holds 8 zero bytes and an HMAC-SHA256 over every
# byte before the HMAC itself (libpgm_pof.so, OSTREAM_HMAC_CRC_TRANSPORT).
# quartus_pfg refuses a .sof if either is wrong (Error 19192, "The SOF file
# content is corrupted").
#
# The HMAC key is HMAC_KEY_1 XOR a 32-byte mask that HMAC_TRANSPORT's
# constructor stores. It is Altera's, so it isn't committed:
# `sof.py key` derives it from the user's own install into specimens/.
#
# The first 0x3a record is the configuration image: u32 0, then the bytes
# that devices/10nm/<die>/sof_meta.pb lays out.
#
#   python3 -I tools/sof.py key                       # once: derive the HMAC key
#   python3 -I tools/sof.py check in.sof
#   python3 -I tools/sof.py image in.sof out.img      # extract the image
#   python3 -I tools/sof.py patch in.sof new.img out.sof
import binascii
import gzip
import hashlib
import hmac
import os
import re
import struct
import subprocess
import sys

_REV = bytes(int(f'{i:08b}'[::-1], 2) for i in range(256))
IMAGE_TAG = 0x3a
CRC_TAG = 0x08
HMAC_TAG = 0x41
POF_LIB = '/pool/dev/altera_pro/quartus-pro/26.1.1/qcore/linux64/libpgm_pof.so'
KEY_FILE = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'keys', 'sof_hmac.key')


def derive_key(lib=POF_LIB):
    """HMAC_KEY_1 XOR the mask HMAC_TRANSPORT::HMAC_TRANSPORT() writes at this+8."""
    nm = subprocess.run(['nm', '-C', lib], capture_output=True, text=True).stdout
    sym = lambda name: int(re.search(rf'^([0-9a-f]+) . {re.escape(name)}$', nm, re.M).group(1), 16)
    key1, ctor = sym('HMAC_KEY_1'), sym('HMAC_TRANSPORT::HMAC_TRANSPORT()')
    dis = subprocess.run(['objdump', '-d', f'--start-address={ctor}', f'--stop-address={ctor + 0x100}', lib],
                         capture_output=True, text=True).stdout
    mask = {int(off, 16): int(v, 16) for v, off in re.findall(r'movb\s+\$0x([0-9a-f]+),0x([0-9a-f]+)\(%rdi\)', dis)}
    data = open(lib, 'rb').read()
    return bytes(data[key1 + i] ^ mask[8 + i] for i in range(32))


def key():
    return open(KEY_FILE, 'rb').read()


def crc16_x25(data):
    # crc_hqx is the MSB-first CCITT CRC; bit-reversing every byte and the
    # result turns it into the reflected one at C speed.
    c = binascii.crc_hqx(data.translate(_REV), 0xffff)
    return int(f'{c:016b}'[::-1], 2) ^ 0xffff


def records(raw):
    """[(tag, data_offset, length)] of an uncompressed .sof."""
    assert raw[:4] == b'SOF\0', 'not a SOF'
    out, p = [], 12
    while p < len(raw):
        tag, ln = struct.unpack_from('<HI', raw, p)
        out.append((tag, p + 6, ln))
        p += 6 + ln
    return out


def load(path):
    raw = open(path, 'rb').read()
    return gzip.decompress(raw) if raw[:2] == b'\x1f\x8b' else raw


def image(raw):
    tag, off, ln = next(r for r in records(raw) if r[0] == IMAGE_TAG)
    return raw[off + 4:off + ln]


def with_image(raw, img):
    tag, off, ln = next(r for r in records(raw) if r[0] == IMAGE_TAG)
    assert len(img) == ln - 4, 'image size differs'
    b = bytearray(raw)
    b[off + 4:off + ln] = img
    return resign(b)


def resign(b):
    tag, off, ln = next(r for r in records(b) if r[0] == HMAC_TAG)
    assert ln == 40, 'HMAC record is not 8 + 32 bytes'
    b[off + 8:off + 40] = hmac.new(key(), bytes(b[:off + 8]), hashlib.sha256).digest()
    tag, off, ln = records(b)[-1]
    assert tag == CRC_TAG and ln == 2, 'last record is not the CRC'
    b[off:off + 2] = struct.pack('<H', crc16_x25(bytes(b[:off])))
    return bytes(b)


if __name__ == '__main__':
    cmd = sys.argv[1]
    if cmd == 'key':
        os.makedirs(os.path.dirname(KEY_FILE), exist_ok=True)
        open(KEY_FILE, 'wb').write(derive_key(*sys.argv[2:3]))
    elif cmd == 'image':
        open(sys.argv[3], 'wb').write(image(load(sys.argv[2])))
    elif cmd == 'patch':
        new = with_image(load(sys.argv[2]), open(sys.argv[3], 'rb').read())
        open(sys.argv[4], 'wb').write(gzip.compress(new, 6))
    elif cmd == 'check':
        raw = load(sys.argv[2])
        print('CRC ok' if resign(bytearray(raw)) == raw else 'CRC BAD')
