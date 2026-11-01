create_clock -name r0 -period 20.0000 [get_ports r0]
create_clock -name r1 -period 20.0000 [get_ports r1]
create_clock -name r2 -period 20.0000 [get_ports r2]
create_clock -name r3 -period 40.0000 [get_ports r3]
derive_pll_clocks
