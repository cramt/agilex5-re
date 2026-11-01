# Derive the I/O pin template, db/sm4revb/io/template.tsv (README (p)), from
# the I/O bits of training images: what `a5db idle-diff` reports in hviowr and
# io96 instances and the unnamed io96 header bits, for designs whose pins and
# their use are known.
#
#   python3 -I tools/pins/derive.py <out.tsv> <idle-diff>:<pin>=<use>[,<pin>=<use>...] ...
#
# <pin> is a DE25-Nano ball (designs/pins/de25_pins.tsv), <use> in, out,
# clock_in, in_reg or out_reg (an I/O register on the pin). Every I/O bit of every training image must be explained by exactly
# one template row, and the rows must agree between pins of the same kind;
# the script stops otherwise.
#
# Template rows (`block scope use std uid bit value`, see docs/db-format.md):
#   hviowr pin   per used HVIO pin; {pin} = xx20_u<b>/xx4_u<g>/xx1_u<k> from the
#                pad name HVIO_B<b>_<g>_<k>, {bank} = xx20_u<b>, {n} = 4g + k
#   io96   pin   per used IO96 pin, {L} = lane, {p} = pin in the lane (pad
#                P<12L + p>)
#   io96   lane_any / lane_out_only / lane_in   per lane with any used pin /
#                outputs and no inputs / at least one input
#   io96   bank  once per IO96 bank with any used pin
# {obj} is the bank's image object (io96_0 for pio_0_0, io96_1 for pio_1_0)
# for rows that set an unnamed header bit (uid @{obj}/hdr, bit = offset).
# An HVIO clock pin's ioclk/ss clock mux is a rule in a5db (src/pins.rs).
# A bit field like `iossm_mem_init_122_bit[{28992+32*L+p}]` is evaluated with
# the pin's L, p (integers, + and *, no parentheses).
import os
import re
import sys

here = os.path.dirname(os.path.abspath(__file__))
repo = os.path.dirname(os.path.dirname(here))

PINS = {}
for line in open(os.path.join(repo, 'designs/pins/de25_pins.tsv')):
    if line.startswith(('#', 'signal\t')):
        continue
    sig, ball, std, bank, block, pad = line.rstrip('\n').split('\t')
    PINS[ball] = (sig, std, block, pad)


def pin_info(ball):
    sig, std, block, pad = PINS[ball]
    m = re.match(r'HVIO_B(\d)_(\d)_(\d)_', pad)
    if m:
        b, g, k = map(int, m.groups())
        return dict(kind='hviowr', uid=block, std=std, b=b, g=g, k=k,
                    path=f'xx20_u{b}/xx4_u{g}/xx1_u{k}', bank=f'xx20_u{b}', n=4 * g + k)
    m = re.match(r'P(\d+)X0Y0', pad)
    n = int(m.group(1))
    return dict(kind='io96', uid=block, std='1.1-V' if std == 'DPHY' else std, L=n // 12, p=n % 12)


def load(path):
    bits = {}
    for line in open(path):
        f = line.rstrip('\n').split('\t')
        if len(f) < 3:
            continue
        if f[0].startswith('unnamed '):
            for kv in f[2].split(' '):
                if kv:
                    off, v = kv.rsplit('=', 1)
                    bits[('@' + f[0].split()[1], off)] = v
            continue
        if not f[0].startswith(('hvio', 'pio')):
            continue
        for kv in f[2].split(' '):
            if kv:
                b, v = kv.rsplit('=', 1)
                bits[(f[0], b)] = v
    return bits


# IO96 rows whose bit index is a function of lane and pin (README (p)).
IOSSM = 'x0/xiocenter_wrap/xiossmwr/iossm_mem_init_122_bit'
LANE_WORD = 28992  # 32 bits per lane from lane 4's word: inputs at +p, outputs at +12+p
LANE_FLAG = 28800  # bit L of the bytes at +0, +8, +16, +24 marks a used lane
# io96 header bit of an output, base + 229 L + p: seen in lanes 4 and 5 only
# (the DE25-Nano's), so the stride is [inf] for the other lanes.
HDR_OUT = 45136


def classify_io96(uid, bit, used):
    """(scope, use, generic bit) for an io96 bit, or None."""
    lanes = {u['L'] for u in used}
    m = re.match(re.escape(IOSSM) + r'\[(\d+)\]$', bit)
    if m:
        i = int(m.group(1))
        for u in used:
            w = LANE_WORD + 32 * (u['L'] - 4)
            if u['use'] in ('in', 'clock_in', 'in_reg') and i == w + u['p']:
                return 'pin', u['use'], IOSSM + '[{28864+32*L+p}]'
            if u['use'] == 'out' and i == w + 12 + u['p']:
                return 'pin', 'out', IOSSM + '[{28876+32*L+p}]'
        for L in lanes:
            for k in range(4):
                if i == LANE_FLAG + 8 * k + L:
                    return 'lane_any', '*', IOSSM + f'[{{{LANE_FLAG + 8 * k}+L}}]'
        return 'bank', '*', bit
    if uid.startswith('@'):
        for u in used:
            if u['use'] == 'out' and int(bit) == HDR_OUT + 229 * u['L'] + u['p']:
                return 'pin', 'out', f'{{{HDR_OUT}+229*L+p}}'
        return None
    if 'rd_training' in bit:
        return 'bank', '*', bit
    for u in used:
        L, p = u['L'], u['p']
        g = re.sub(rf'xio12phytop_{L}/io12phy/(\S*?)_{p}(?=[/\[]|$)', r'xio12phytop_{L}/io12phy/\1_{p}', bit)
        g = re.sub(rf'xiophyadaptwr_{L}/x_gpio_{p}/', 'xiophyadaptwr_{L}/x_gpio_{p}/', g)
        if g != bit:
            return 'pin', u['use'], g
    for L in lanes:
        if f'xio12phytop_{L}/io12phyctrl_wrp/' in bit or f'xiofbradapt/xio12_{L}/' in bit:
            return 'lane', '*', bit.replace(f'xio12phytop_{L}/', 'xio12phytop_{L}/').replace(f'xio12_{L}/', 'xio12_{L}/')
    return 'bank', '*', bit


def main():
    out = sys.argv[1]
    rows = {}  # (block, scope, use, std, uid, bit) -> value
    lane_obs = []  # (kind of lane, {generic bit: value}) for the lane rows

    def add(key, v, where):
        if rows.setdefault(key, v) != v:
            sys.exit(f'{where}: {key} is {v} here, {rows[key]} elsewhere')

    for spec in sys.argv[2:]:
        path, pins = spec.split(':', 1)
        used = []
        for pu in pins.split(','):
            ball, use = pu.split('=')
            u = pin_info(ball)
            u['use'] = use
            u['ball'] = ball
            used.append(u)
        bits = load(path)
        lanes = {}
        for (uid, bit), v in bits.items():
            hv = [u for u in used if u['kind'] == 'hviowr' and u['uid'] == uid]
            if uid.startswith('hvio'):
                hit = [u for u in hv if bit.startswith(u['path'] + '/')]
                if hit:
                    u = hit[0]
                    rel = bit[len(u['path']) + 1:]
                    std = u['std'] if 'iio_config' in rel else '*'
                    add(('hviowr', 'pin', u['use'], std, '{uid}', '{pin}/' + rel), v, path)
                    continue
                hit = [u for u in hv if u['use'] == 'clock_in' and bit == f"{u['bank']}/xx20_clkordat_din_1to2_demux_u{u['n']}/cr_clkordat"]
                if hit:
                    add(('hviowr', 'pin', 'clock_in', '*', '{uid}', '{bank}/xx20_clkordat_din_1to2_demux_u{n}/cr_clkordat'), v, path)
                    continue
                if bit.startswith('xif/xif_clk_mux/') and any(u['use'] == 'clock_in' for u in hv):
                    # The bank's ioclk and ss clock muxes: a rule in a5db's
                    # pins.rs, checked by tools/pins/check.sh (README (r)).
                    continue
                sys.exit(f'{path}: {uid} {bit}={v} is not a used pin\'s')
            io = [u for u in used if u['kind'] == 'io96']
            c = classify_io96(uid, bit, io)
            if c is None:
                sys.exit(f'{path}: {uid} {bit}={v} not explained')
            scope, use, g = c
            if scope == 'lane':
                L = int(re.search(r'(?:xio12phytop_|xio12_)(\d+)/', bit).group(1))
                lanes.setdefault(L, {})[g] = v
                continue
            uid_t = '@{obj}/' + uid.split('/')[1] if uid.startswith('@') else '{uid}'
            add(('io96', scope, use, '*', uid_t, g), v, path)
        for L, gbits in lanes.items():
            uses = {u['use'] for u in used if u['kind'] == 'io96' and u['L'] == L}
            lane_obs.append(('in' in uses, gbits, f'{path} lane {L}'))
    # Lane rows: bits every observed lane has alike are lane_any; the rest
    # belong to output-only lanes or to lanes with an input.
    common = None
    for _, g, _ in lane_obs:
        items = set(g.items())
        common = items if common is None else common & items
    for has_in, g, where in lane_obs:
        for b, v in g.items():
            scope = 'lane_any' if (b, v) in common else ('lane_in' if has_in else 'lane_out_only')
            add(('io96', scope, '*', '*', '{uid}', b), v, where)
    with open(out, 'w') as f:
        f.write('# I/O pin template (README (p)), written by tools/pins/derive.py from:\n')
        for spec in sys.argv[2:]:
            f.write(f'#   {os.path.basename(spec.split(":")[0])}: {spec.split(":", 1)[1]}\n')
        f.write('block\tscope\tuse\tstd\tuid\tbit\tvalue\n')
        for k in sorted(rows):
            f.write('\t'.join(k) + f'\t{rows[k]}\n')
    print(f'{len(rows)} rows, {len(lane_obs)} lanes observed', file=sys.stderr)


if __name__ == '__main__':
    main()
