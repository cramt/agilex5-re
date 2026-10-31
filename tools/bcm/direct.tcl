# DIRECT attributes of block types: the bit names of every attribute that has
# no setting list (asm2's get_attribute_values is empty), MSB first, as
# db/sm4revb/direct.tsv stores them (README (q)).
#   quartus_asm -t direct.tcl <out.tsv> <type>...
load_package asm2
load_devices
set out [open [lindex $argv 0] w]
foreach t [lrange $argv 1 end] {
    foreach a [get_attribute_names sm4revb $t] {
        puts stderr "$t $a"
        if {[catch {get_attribute_values sm4revb $t $a} v] || [llength $v] > 0} { continue }
        if {[catch {get_bit_names_for_attribute sm4revb $t $a} bits] || [llength $bits] == 0} { continue }
        puts $out "$t\t$a\t[join $bits { }]"
        flush $out
    }
}
close $out
