# Routes between the fabric and the HPS atom, per port bit, for mapping its
# ports to routing-graph nodes (db/sm4revb/hps/lwhps2fpga.tsv). Run with
# `tools/quartus.sh quartus_fit <project> --eco hps_routes.tcl` in a compiled
# project. report_routing takes a port type, not a bit, unless the port is
# written TYPE[bit]; HPS outputs are traced to each fanout's D/data port. Lines:
#   I <type> <bit> fanin {..} <src>   O <type> <bit> -> <dest> <port>
#   T <to_port>, then R hops, X error, E end
read_atom_netlist
set out [open hps_routes.txt w]
proc dump {out to port} {
    puts $out "T $port"
    if {[catch {report_routing -to $to -to_port $port -return_result} r]} {
        puts $out "X $r"
    } else {
        foreach hop [lrange $r 1 end] { puts $out "R [join $hop { }]" }
    }
    puts $out "E"; flush $out
}
foreach_in_collection a [get_atom_nodes] {
    set name [get_atom_node_info -node $a -key NAME]
    set type [get_atom_node_info -node $a -key TYPE]
    if {$type ne "SOC_HPS"} { continue }
    puts $out "A $name $type"
    foreach p [get_atom_iports -node $a] { lappend it [get_atom_port_info -node $a -port_id $p -type iport -key TYPE] }
    foreach p [get_atom_oports -node $a] { lappend ot [get_atom_port_info -node $a -port_id $p -type oport -key TYPE] }
    foreach t [lsort -unique $it] { puts $out "PI $t [llength [lsearch -all -exact $it $t]]" }
    foreach t [lsort -unique $ot] { puts $out "PO $t [llength [lsearch -all -exact $ot $t]]" }
    flush $out
    array unset bit
    foreach p [get_atom_iports -node $a] {
        set pt [get_atom_port_info -node $a -port_id $p -type iport -key TYPE]
        if {![info exists bit($pt)]} { set bit($pt) 0 }
        set k $bit($pt); incr bit($pt)
        if {![string match "LWSOC2FPGA_*" $pt] && $pt ne "F2S_FPGA_IRQ"} { continue }
        set src ""
        catch {set src [get_atom_port_info -node $a -port_id $p -type iport -key FANIN]}
        if {$src eq "" || [lindex $src 0] < 0} { continue }
        set sname ""
        catch {set sname [get_atom_node_info -node [lindex $src 0] -key NAME]}
        puts $out "I $pt $k fanin {$src} $sname"
        # Whole-type and per-bit forms: which one report_routing resolves.
        dump $out $name "$pt\[$k\]"
    }
    array unset bit
    foreach p [get_atom_oports -node $a] {
        set pt [get_atom_port_info -node $a -port_id $p -type oport -key TYPE]
        if {![info exists bit($pt)]} { set bit($pt) 0 }
        set k $bit($pt); incr bit($pt)
        if {![string match "LWSOC2FPGA_*" $pt]} { continue }
        set fo {}
        catch {set fo [get_atom_port_info -node $a -port_id $p -type oport -key FANOUT]}
        foreach f $fo {
            set dn [lindex $f 0]; set dp [lindex $f 1]
            set dname ""; set dpt ""
            catch {set dname [get_atom_node_info -node $dn -key NAME]}
            catch {set dpt [get_atom_port_info -node $dn -port_id $dp -type iport -key TYPE]}
            puts $out "O $pt $k -> $dname $dpt"
            dump $out $dname $dpt
        }
    }
}
close $out
