# Dump every BCM intent's CUSTOM attributes: for each setting, the named bits
# and their values. Run under quartus_sh:
#   quartus_sh -t settings.tcl <device> <out.tsv>
# One line per (intent, attribute, setting): bits and values, both as Tcl
# lists. A value "<0>" is how the API prints a don't-care-looking bit; it is
# kept verbatim.
lassign $quartus(args) dev out
load [file join $quartus(binpath) libddb_dbcm_tcl.so] Bcm_device
read_bcm_device -device $dev
set seen [dict create]
set todo [get_bcm_intents -top_level]
set f [open $out w]
while {[llength $todo]} {
    set todo [lassign $todo i]
    if {[dict exists $seen $i]} continue
    dict set seen $i 1
    foreach c [get_bcm_intents -children $i] { lappend todo $c }
    foreach a [get_attributes -bcm_intent $i] {
        set info [get_attribute_info -intent $i -name $a]
        if {[dict get $info Type] ne "CUSTOM"} {
            puts $f [join [list $i $a [dict get $info Type] - [dict get $info] -] \t]
            continue
        }
        if {[catch {get_attribute_bits -bcm $i -attribute $a} bits]} { set bits ? }
        foreach s [get_settings -bcm_intent $i -attribute $a] {
            if {[catch {get_setting_bit_values -bcm $i -attribute $a -setting $s} v]} { set v ? }
            puts $f [join [list $i $a CUSTOM $s $bits $v] \t]
        }
    }
}
close $f
puts "intents: [dict size $seen]"
