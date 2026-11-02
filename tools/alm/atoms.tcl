# Dump the fitted atom netlist of a compiled design, for the ALM model
# (README (w)). Run in quartus_fit's ECO mode on a copy of a run:
#   quartus_fit <rev> --eco tools/alm/atoms.tcl      (writes atoms.txt)
# Per atom: "A <name> <type> <location>", "K <key> <value>" for every key
# get_atom_node_info knows, then per input port "I <port> <fanin atom> <fanin port>".
read_atom_netlist
set out [open atoms.txt w]
set keys {}
if {[catch {get_atom_node_info -node [lindex [get_atom_nodes] 0] -key NO_SUCH_KEY} msg]} { puts $out "# keys: $msg" }
foreach_in_collection a [get_atom_nodes] {
    set name [get_atom_node_info -node $a -key NAME]
    set type [get_atom_node_info -node $a -key TYPE]
    set loc ?
    catch {set loc [get_atom_node_info -node $a -key LOCATION]}
    puts $out "A $name $type $loc"
    # The LUT mask key is LUTMASK_LUT_MASK (libtss_astr.so's key names: a type
    # prefix, then the name); LUT_MASK isn't one.
    foreach k {LUTMASK_LUT_MASK EXTENDED_LUT SHARED_ARITH LUT_INPUT_PERMUTATION ATOM_TYPE WYSIWYG_TYPE} {
        if {![catch {get_atom_node_info -node $a -key $k} v]} { puts $out "K $k $v" }
    }
    foreach p [get_atom_iports -node $a] {
        set ptype [get_atom_port_info -node $a -port_id $p -type iport -key TYPE]
        if {[catch {get_atom_port_info -node $a -port_id $p -type iport -key FANIN} src]} { continue }
        if {$src eq "" } { continue }
        set sname ?; set sport ?
        catch {set sname [get_atom_node_info -node [lindex $src 0] -key NAME]}
        catch {set sport [get_atom_port_info -node [lindex $src 0] -port_id [lindex $src 1] -type oport -key TYPE]}
        puts $out "I $ptype $sname $sport"
    }
}
close $out
