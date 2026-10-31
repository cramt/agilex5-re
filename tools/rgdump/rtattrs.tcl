# Print every attribute of the given block instances in a .sof, one per line:
#   <uid> TAB <attribute> TAB <value>
#   quartus_asm -t rtattrs.tcl <sof> <uid>...
load_package asm2
load_devices
set h [open_sof [lindex $argv 0]]
foreach uid [lrange $argv 1 end] {
    set type [regsub {_r[0-9]+_c[0-9]+$} $uid {}]
    foreach a [get_attribute_names sm4revb $type] {
        if {[catch {get_attribute $h $uid $a} v]} { set v "<ERR>" }
        puts "$uid\t$a\t$v"
    }
}
