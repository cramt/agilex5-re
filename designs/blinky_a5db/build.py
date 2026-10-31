"""Blinky with no Quartus in its history (README (n)): LED0 toggles every
2^24 cycles of CLOCK1_50, about 1.5 Hz, like designs/blinky (no KEY0 reset).

Everything is placed and routed by hand through a5db, on the idle image the
database describes:

  a5db blank                 the idle image in a5db's own .sof container
  a5db apply                 CLOCK1_50 (V16) input and LED0 (DF35) output
  a5db set (x3 LABs)         LUT masks and flip-flop settings
  a5db net                   the clock tree and every data net
  a5db wrap                  a fresh container for the final image
  a5db rbf                   the .rbf

The counter is synchronous, 25 flip-flops q0..q24 in groups of five: q_i
toggles when q_5g..q_(i-1) and the group carry a_g are all 1, a_(g+1) =
a_g & q_5g..q_5g+4 (a_0 = 1). 25 LUT+FF ALMs and 4 carry LUTs (output through
the ALM's second register path in wire mode, as Quartus does for a plain LUT)
fill three LABs at X107, Y6..Y8, next to LED0's fabric entry at X107 Y5.

  python3 -I designs/blinky_a5db/build.py <out dir>
"""
import os
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
A5DB = os.path.join(REPO, 'tools/a5db/target/release/a5db')
DB = os.path.join(REPO, 'db/sm4revb')
GRAPH = os.path.join(REPO, 'specimens/rg/full')
QUARTUS = '/pool/dev/altera_pro/quartus-pro/26.1.1'
SOF_KEY = os.path.join(REPO, 'specimens/sof_hmac.key')
RBF_KEY = os.path.join(REPO, 'specimens/rbf_keyed_hash.key')

X, ROWS = 107, (6, 7, 8)
# LAB_RE pins of ALM k (README (n)): LUT inputs A..F, reg0/xreg0 and
# reg0/xreg1 outputs, the LAB clock.
IN = {'A': 3, 'B': 4, 'C': 2, 'D': 1, 'E': 0, 'F': 7}
def in_pin(k, port): return 8 * k + IN[port]
def q_pin(k): return 89 + 4 * k
def wire_pin(k): return 90 + 4 * k
LAB_CLOCK = 83
CLOCK_SRC = 'HVIOWR_RE:X121Y30Z500I2'   # V16's clock out of hvio_3_1 (V16_clock_in.tsv)
CLOCK_VIA = ['DCM_mux:X121Y31Z0I2', 'CE_CC_CLKMUX41_LOW_INPUT:X121Y51Z111I2', 'Switchbox_clock_mux:X121Y51Z0I279']
LED_SINK = 'IO96_RE:X61Y0Z0I3959'       # behind the row-5 RPI at X107 (DF35_out.tsv)


def alm_path(k):
    return f'xtophalf/xale{k}' if k < 5 else f'xbothalf/xale{k - 5}'


def stored_mask(fn):
    """asm2's DIRECT lut_mask, MSB first, for a function of the WYSIWYG
    index i (bit 0 = DATAA ... bit 5 = DATAF): stored[j] = not W[j ^ 31]."""
    w = [fn(i) for i in range(64)]
    stored = [1 - w[j ^ 31] for j in range(64)]
    return ''.join(str(stored[63 - c]) for c in range(64))


def cells():
    """(name, inputs) in placement order; inputs are signal names on A, B, ..."""
    out = []
    for g in range(5):
        for r in range(5):
            i = 5 * g + r
            ins = [f'q{i}'] + ([f'a{g}'] if g else []) + [f'q{5 * g + s}' for s in range(r)]
            out.append((f'q{i}', ins))
        if g < 4:
            out.append((f'a{g + 1}', ([f'a{g}'] if g else []) + [f'q{5 * g + s}' for s in range(5)]))
    return out


def main(outdir):
    os.makedirs(outdir, exist_ok=True)
    run = lambda *a: subprocess.run([A5DB, *a], check=True)
    cs = cells()
    place = {name: (ROWS[n // 10], n % 10) for n, (name, _) in enumerate(cs)}
    node = lambda row, pin: f'LAB_RE:X{X}Y{row}Z0I{pin}'
    src = {name: node(row, q_pin(k) if name[0] == 'q' else wire_pin(k)) for name, (row, k) in place.items()}

    sof = [os.path.join(outdir, f'{n}.sof') for n in ('0_blank', '1_io', '2_lab6', '3_lab7', '4_lab8', '5_nets', 'blinky_a5db')]
    run('blank', DB, QUARTUS, SOF_KEY, sof[0])
    run('apply', DB, sof[0], sof[1], SOF_KEY, os.path.join(DB, 'io/V16_clock_in.tsv'), os.path.join(DB, 'io/DF35_out.tsv'))
    for li, row in enumerate(ROWS):
        attrs = ['xsecmux/xcw_la_lcb_clkgen/xlabclk0/smxclk_tie_off=DISABLE_TIEOFF']
        for name, ins in cs:
            r, k = place[name]
            if r != row:
                continue
            P = alm_path(k)
            if name[0] == 'q':
                fn = lambda i, n=len(ins): (i & 1) ^ int(all(i >> b & 1 for b in range(1, n)))
                reg = [f'{P}/xcw_la_le_reg0/xlabclk_mux/regclk=DFF_LABCLK0',
                       f'{P}/xcw_la_le_reg0/xreg0/reg_input=LUTOUT_6LUT',
                       f'{P}/xcw_la_le_reg0/xreg0/register_comb=DFF_LATCH',
                       f'{P}/xcw_la_le_reg0/xreg0/leout=LEOUT']
            else:
                fn = lambda i, n=len(ins): int(all(i >> b & 1 for b in range(n)))
                reg = [f'{P}/xcw_la_le_reg0/xreg1/reg_input=LUTOUT_6LUT',
                       f'{P}/xcw_la_le_reg0/xreg1/register_comb=WIRE_MODE',
                       f'{P}/xcw_la_le_reg0/xreg1/leout=LEOUT']
            attrs += [f'{P}/xcw_la_le_lut/lut_mask={stored_mask(fn)}', *reg,
                      f'{P}/xcw_la_le_reg0/xreg_aclr/regclr=CLR2',
                      f'{P}/xcw_la_le_reg1/xreg_aclr/regclr=CLR2',
                      f'{P}/xcw_la_le_regctrl/xcw_la_le_regctrl_reg/ce_sel=CE',
                      f'{P}/xcw_la_le_regctrl/xcw_la_le_regctrl_reg/clr_sel=CLR']
        run('set', DB, sof[1 + li], sof[2 + li], SOF_KEY, f'cw_la_lab_r{row}_c{X}', *attrs)
    sinks = {}
    for name, ins in cs:
        r, k = place[name]
        for port, sig in zip('ABCDEF', ins):
            sinks.setdefault(sig, []).append(node(r, in_pin(k, port)))
    sinks.setdefault('q24', []).append(LED_SINK)
    # The clock takes the path Quartus uses for V16 in blinky, fz0 and rt0 up
    # to the switchbox (DCM output 2, the Y51 P2C mux 8 input 2), then a5db's
    # own tree to the three LAB clocks.
    nets = ['clock:' + CLOCK_SRC + '>' + CLOCK_VIA[0], *(f'clock:{a}>{b}' for a, b in zip(CLOCK_VIA, CLOCK_VIA[1:])),
            'clock:' + CLOCK_VIA[-1] + '>' + ';'.join(node(r, LAB_CLOCK) for r in ROWS)]
    nets += [src[s] + '>' + ';'.join(t) for s, t in sinks.items()]
    with open(os.path.join(outdir, 'net.log'), 'w') as log:
        subprocess.run([A5DB, 'net', DB, GRAPH, sof[4], sof[5], SOF_KEY, *nets], check=True, stdout=log)
    # A fresh container for the final image: design hash and record sizes.
    run('wrap', QUARTUS, SOF_KEY, sof[5], sof[6])
    run('rbf', DB, sof[6], os.path.join(outdir, 'blinky_a5db.rbf'), QUARTUS, RBF_KEY)


if __name__ == '__main__':
    main(sys.argv[1])
