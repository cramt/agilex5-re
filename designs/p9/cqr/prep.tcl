# The CPU's register file in flops, as a5pnr's yosys flow maps it, so the
# ALM count compares like with like (README (z3)): femtorv32_quark.v with a
# ramstyle "logic" attribute on registerFile, written into the run directory.
set f [open ../../../designs/p7/cpu/femtorv32_quark.v r]; set src [read $f]; close $f
set n [regsub {reg \[31:0\] registerFile \[31:0\];} $src {(* ramstyle = "logic" *) reg [31:0] registerFile [31:0];} src]
if {$n != 1} { error "registerFile declaration not found" }
set f [open femtorv32_logic.v w]; puts -nonewline $f $src; close $f
