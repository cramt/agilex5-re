# Dump every routed connection of a compiled design, for checking the
# routing join (a5db routecheck). Run in quartus_fit's ECO mode on a copy:
#   quartus_fit <rev> --eco tools/rgdump/routes.tcl      (writes routes.txt)
# Per connected atom input port: "C <atom> <port>", one "R <location or port>
# <fanout>" line per hop of report_routing, then "E". report_connections
# crashes the fitter on atoms with unconnected inputs, so the ports come from
# the atom netlist instead. Lines are flushed so a crash keeps what was done.
read_atom_netlist
set out [open routes.txt w]
set nconn 0
foreach_in_collection a [get_atom_nodes] {
    set name [get_atom_node_info -node $a -key NAME]
    set type [get_atom_node_info -node $a -key TYPE]
    puts $out "A $name $type"
    # report_routing to a top-level PORT atom crashes the fitter; the I/O
    # buffer atoms behind the ports carry those routes.
    if {$type eq "PORT"} { continue }
    foreach p [get_atom_iports -node $a] {
        set ptype [get_atom_port_info -node $a -port_id $p -type iport -key TYPE]
        if {[catch {get_atom_port_info -node $a -port_id $p -type iport -key FANIN} src] || $src eq "" || $src < 0} {
            continue
        }
        # An FF's D from its own ALM's LUT has no route, and report_routing
        # crashes on it; FF D inputs from routing are lost with it [unk].
        if {$type eq "FF" && $ptype eq "D"} { continue }
        puts $out "C $name $ptype"
        flush $out
        if {[catch {report_routing -to $name -to_port $ptype -return_result} r]} {
            puts $out "X $r"
        } else {
            foreach hop [lrange $r 1 end] { puts $out "R [join $hop { }]" }
        }
        puts $out "E"
        flush $out
        incr nconn
    }
}
close $out
puts "routes.tcl: $nconn connections"
