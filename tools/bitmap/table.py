# Database tables are <name>.tsv, or <name>.tsv.xz when the table is bigger
# than BIG (io96 and sm_hssi_top layouts; README (j)). Readers take either.
import lzma
import os

BIG = 4 << 20


def open_table(path):
    if os.path.exists(path):
        return open(path)
    return lzma.open(path + '.xz', 'rt')


def write_table(path, text):
    for p in (path, path + '.xz'):
        if os.path.exists(p):
            os.remove(p)
    if len(text) > BIG:
        with lzma.open(path + '.xz', 'wt', preset=9) as f:
            f.write(text)
    else:
        with open(path, 'w') as f:
            f.write(text)
