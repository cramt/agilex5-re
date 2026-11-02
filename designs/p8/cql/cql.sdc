# Over-constrained on purpose: the fitter works for speed and quartus_sta
# reports the Fmax it reached (README (z3)).
create_clock -name CLOCK1_50 -period 3.333 [get_ports CLOCK1_50]
