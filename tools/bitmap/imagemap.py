# Where each part of the sm4revb configuration image lives (README (g)).

# sm4revb image layout from sof_meta.pb (README (g)): object, part, start, size.
PARTS = [
    ('perictrl0', 'hdr', 0, 902), ('perictrl0', 'data', 902, 4194304),
    ('io96_0', 'hdr', 4195206, 5855), ('io96_0', 't40', 4201061, 16384), ('io96_0', 't41', 4217445, 529408),
    ('perictrl2', 'hdr', 4746853, 902), ('perictrl2', 'data', 4747755, 4194304),
    ('io96_1', 'hdr', 8942059, 5855), ('io96_1', 't40', 8947914, 16384), ('io96_1', 't41', 8964298, 529408),
    ('perictrl1', 'hdr', 9493706, 2658), ('perictrl1', 'data', 9496364, 4194304),
    ('sector0', 'hdr', 13690668, 412), ('sector0', 'cram', 13691080, 2540160), ('sector0', 'ram', 16231240, 491520),
    ('sector1', 'hdr', 16722760, 284), ('sector1', 'cram', 16723044, 2540160), ('sector1', 'ram', 19263204, 491520),
]
FRAME_BYTES = 540


def addr(off, k):
    """(object, part, a, b): CRAM is (frame, bdata); anything else (byte, bit)."""
    for obj, part, start, size in PARTS:
        r = off - start
        if 0 <= r < size:
            if part == 'cram':
                fr, by = divmod(r, FRAME_BYTES)
                return obj, part, fr, by * 8 + k
            return obj, part, r, k
    return '-', '-', off, k
