# CUSTOM attributes the Bcm_device dump has no bits for, recovered through
# asm2 (README (q)): on one instance of an image, set each value of the
# attribute and record which of the instance's named bits change.
#   quartus_asm -t asm2_probe.tcl <in.sof> <list> <out.tsv> [<first line>]
# Some attributes abort asm2 with an internal error; the output is appended
# and each attribute starts with a "#start <n>" line, so asm2_probe.sh can
# restart after the one that crashed.
# <list>: "<type> <uid> <attribute path>" per line. Output rows:
#   type, uid, attribute, value, bit=v ... (bits that differ from the image)
load_package asm2
load_devices
lassign $quartus(args) sof list out first
if {$first eq ""} { set first 0 }
set h [open_sof $sof]
set dev [get_device_name $h]
set f [open $list]
set o [open $out a]
set n -1
set names [dict create]
while {[gets $f line] >= 0} {
    incr n
    if {$n < $first} continue
    lassign $line btype uid attr
    if {$uid eq ""} continue
    puts $o "#start $n"
    flush $o
    if {![dict exists $names $btype]} {
        dict set names $btype [lmap b [get_bit_names $dev $btype] {expr {[is_bit_defined $h $uid $b] ? $b : [continue]}}]
    }
    set bits [dict get $names $btype]
    set base [lmap b $bits {get_bit $h $uid $b}]
    if {[catch {get_attribute $h $uid $attr} orig]} { puts $o "$btype\t$uid\t$attr\t<ERR $orig>\t"; continue }
    if {[catch {get_attribute_values $dev $btype $attr} vals]} { set vals {} }
    foreach v $vals {
        if {[catch {set_attribute $h $uid $attr $v} e]} { puts $o "$btype\t$uid\t$attr\t$v\t<ERR $e>"; continue }
        set ch {}
        foreach b $bits w $base {
            set x [get_bit $h $uid $b]
            if {$x != $w} { lappend ch "$b=$x" }
            if {$x != $w} { set_bit $h $uid $b $w }
        }
        puts $o "$btype\t$uid\t$attr\t$v\t[join $ch { }]"
        flush $o
    }
    puts $o "$btype\t$uid\t$attr\t=$orig\t"
}
close $o
