# IOPLL counters from frequencies (README (v)): the rule the altera_iopll IP
# follows, fitted on the training sweep (tools/pll/ipsweep.py).
#
#   python3 -I tools/pll/compute.py <ref MHz> <out MHz>[,<out MHz>...]
#
# Prints N, M, the VCO frequency and each output's C. The rule: VCO
# candidates are the multiples of the outputs' least common multiple, from
# 3200 MHz down to 600 MHz; for each, the smallest N with a PFD frequency
# (ref / N) of at least 10 MHz that makes M = VCO / PFD an integer; the first
# candidate that has one wins. C_i = VCO / out_i (at most 512).
from fractions import Fraction
import math
import sys

VCO_MAX, VCO_MIN, PFD_MIN, C_MAX = 3200, 600, 10, 512


def lcm(fs):
    # LCM of rationals: lcm(numerators) / gcd(denominators).
    num = 1
    den = 0
    for f in fs:
        num = num * f.numerator // math.gcd(num, f.numerator)
        den = math.gcd(den, f.denominator)
    return Fraction(num, den)


def parse(s):
    # The IP rounds 33.333333 to 100/3: treat a long repeating decimal as its fraction.
    f = Fraction(s)
    g = f.limit_denominator(1000)
    return g if abs(g - f) < Fraction(1, 10 ** 5) else f


def compute(ref, outs):
    ref = parse(ref)
    outs = [parse(o) for o in outs]
    step = lcm(outs)
    k = math.floor(VCO_MAX / step)
    while k >= 1 and step * k >= VCO_MIN:
        vco = step * k
        for n in range(1, math.floor(ref / PFD_MIN) + 1):
            m = vco * n / ref
            if m.denominator == 1 and all((vco / o).denominator == 1 and vco / o <= C_MAX for o in outs):
                return n, int(m), vco, [int(vco / o) for o in outs]
        k -= 1
    return None


if __name__ == '__main__':
    r = compute(sys.argv[1], sys.argv[2].split(','))
    if r is None:
        print('no exact solution')
    else:
        n, m, vco, cs = r
        print(f'N={n} M={m} VCO={float(vco):g} C={cs}')
