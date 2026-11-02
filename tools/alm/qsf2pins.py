# A design's pins file (port, ball, volts: tools/a5pnr's format) from the
# location and I/O standard assignments of its Quartus .qsf.
#   python3 -I tools/alm/qsf2pins.py <design.qsf>
import re
import sys

loc, std, default = {}, {}, None
for l in open(sys.argv[1]):
    m = re.match(r'set_location_assignment PIN_(\w+) -to (\S+)', l)
    if m:
        loc[m.group(2)] = m.group(1)
    m = re.match(r'set_instance_assignment -name IO_STANDARD "([\d.]+)-V[^"]*" -to (\S+)', l)
    if m:
        if m.group(2) == '*':
            default = m.group(1)
        else:
            std[m.group(2)] = m.group(1)
for port, ball in loc.items():
    bus = port.split('[')[0] + '[*]'
    print(port, ball, std.get(port, std.get(bus, default)))
