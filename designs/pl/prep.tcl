# Generate the IOPLL IP (README (r)) into the run directory, before synthesis:
# a 50 MHz reference, one 100 MHz output, locked. The generated files are
# Altera IP, so they are made in the run (tools/flow.sh runs prep.tcl) and
# never committed. ip-deploy and qsys-generate are not chipsmith tools, so
# quartus_sh runs them.
set root [file normalize [file join $::quartus(binpath) .. ..]]
set ipd [file join $root qsys bin ip-deploy]
set qg [file join $root qsys bin qsys-generate]
if {[catch {exec $ipd --component-name=altera_iopll --output-name=pll --family=Agilex\ 5 --part=A5EB013BB23BE4SCS --component-parameter=gui_reference_clock_frequency=50.0 --component-parameter=gui_number_of_clocks=1 --component-parameter=gui_output_clock_frequency0=100.0 --component-parameter=gui_use_locked=true 2>@1} r]} { puts "ip-deploy failed: $r"; exit 1 }
if {[catch {exec $qg pll.ip --synthesis=VERILOG --family=Agilex\ 5 --part=A5EB013BB23BE4SCS 2>@1} r]} { puts "qsys-generate failed: [string range $r end-2000 end]"; exit 1 }
