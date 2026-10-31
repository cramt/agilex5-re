# Read named attributes of a .sof through Quartus's own assembler (asm2):
#   quartus_asm -t getattrs.tcl <sof> <list>     list: "<uid> <attribute>" per line
# Prints "<uid> TAB <attribute> TAB <value>".
load_package asm2
load_devices
set h [open_sof [lindex $argv 0]]
set f [open [lindex $argv 1]]
while {[gets $f line] >= 0} {
    if {[llength $line] < 2} { continue }
    lassign $line uid attr
    if {[catch {get_attribute $h $uid $attr} v]} { set v "<ERR: $v>" }
    puts "$uid\t$attr\t$v"
}
close $f
