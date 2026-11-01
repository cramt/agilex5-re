# An IOPLL as a5db design-file lines (README (v)): the common template
# (db/sm4revb/io/iopll.tsv) plus what follows from the frequencies:
# compute.py's N, M, C; C and N as hi = ceil(x/2), lo = floor(x/2) with the
# odd-duty bit for odd x; M as the 9-bit `gdiv` hi field; the reference mux
# input from the reference pin; output enables; charge-pump trims by PFD
# frequency (three seen: 50, 25 and 10 MHz).
#
#   python3 -I tools/pll/iopll.py <ref ball> <ref MHz> <out MHz>[,<out MHz>...]
#
# The PLL is the one in the reference ball's hviowr instance
# (db/sm4revb/io/pins.tsv); only bank-half 1 reference pins (pad
# HVIO_B1_2_<k>, the PLLREFCLK pins of the DE25-Nano) are modelled.
import os
import re
import sys

here = os.path.dirname(os.path.abspath(__file__))
repo = os.path.dirname(os.path.dirname(here))
sys.path.insert(0, here)
from compute import compute  # noqa: E402

P = 'xpllwrap_top/xpllwrap_bf/xiopllwrap_top/xbf/'
C = P + 'xiopll_custom/'
# Charge-pump current trims and lock filter by PFD frequency (MHz), from
# pl and pa: (cpi_trim[4:0], cpp_trim[4:0], cr_lockf_early[3:0]).
TRIMS = {50: ('00110', '01001', '1000'), 25: ('00011', '00011', '1000'), 10: ('00010', '00010', '1011')}


def lines(ball, ref, outs):
    uid = pad = None
    for l in open(os.path.join(repo, 'db/sm4revb/io/pins.tsv')):
        f = l.split('\t')
        if f[0] == ball:
            uid, pad = f[1], f[2]
    m = re.match(r'HVIO_B1_2_([01])_', pad or '')
    if not m:
        raise SystemExit(f'{ball}: not a bank-half 1 PLL reference pin (HVIO_B1_2_0/1)')
    n, mm, vco, cs = compute(ref, outs)
    pfd = float(ref) / n
    if round(pfd) not in TRIMS:
        raise SystemExit(f'PFD {pfd} MHz: no charge-pump trims known (only {sorted(TRIMS)})')
    used = len(cs)
    out = []
    for l in open(os.path.join(repo, 'db/sm4revb/io/iopll.tsv')):
        if l.startswith('#'):
            continue
        f = l.rstrip('\n').split('\t')
        # Counters 2..6 idle bypassed; a used one isn't.
        k = re.search(r'xccnt_iopll(\d)/dprio_c_m_cnt_bypass_en', f[2])
        if k and int(k.group(1)) < used:
            continue
        out.append('\t'.join([f[0], uid] + f[2:]))
    attr = lambda path, v: out.append(f'attr\t{uid}\t{path}\t{v}')
    direct = lambda path, v, w: out.append(f'direct\t{uid}\t{path}\t{v:0{w}b}')
    bits = lambda path, s: out.extend(f'bit\t{uid}\t{path}[{i}]\t{c}' for i, c in enumerate(reversed(s)))
    attr('xpllwrap_top/xpllwrap_10to1_mux/hviowr_pllwrap_clk_mux_sel', f'PLL_CLKMUX_IOCLKIN_{2 + int(m.group(1))}')
    for k in range(max(2, used)):
        c = cs[k] if k < used else 510
        cnt = f'{C}xccnt_iopll{k}/'
        direct(cnt + 'dprio_c_m_cnt_hi_div_user', (c + 1) // 2, 8)
        direct(cnt + 'dprio_c_m_cnt_lo_div_user', c // 2, 8)
        if c % 2:
            attr(cnt + 'dprio_c_m_cnt_even_duty_en', 'DPRIO_CNT_EVEN_DUTY_ENABLED')
        if 1 <= k < used:
            attr(f'{C}xpllcoutbuf{k}/cnt_out_en', 'CNT_OUTPUT_ENABLE')
    direct(C + 'xiopll_core/gdiv/dprio_c_m_cnt_hi_div_user', mm, 9)
    if n > 1:
        nc = C + 'xiopll_core/xncnt_iopll/'
        attr(nc + 'pll_n_cnt_bypass_en', 'PLL_N_CNT_DIV_ENABLED')
        direct(nc + 'pll_n_cnt_hi_div', (n + 1) // 2, 8)
        direct(nc + 'pll_n_cnt_lo_div', n // 2, 8)
        if n % 2:
            attr(nc + 'pll_n_cnt_odd_div_duty_en', 'PLL_N_CNT_EVEN_DUTY_ENABLED')
    cpi, cpp, lockf = TRIMS[round(pfd)]
    bits(C + 'xiopll_core/ipllljtopana/xchgpmplf/cpi_trim', cpi)
    bits(C + 'xiopll_core/ipllljtopana/xchgpmplf/cpp_trim', cpp)
    bits(P + 'xfpll_ctrl/cr_lockf_early', lockf)
    return out


if __name__ == '__main__':
    print('\n'.join(lines(sys.argv[1], sys.argv[2], sys.argv[3].split(','))))
