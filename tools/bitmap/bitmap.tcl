# Map every named configuration bit of one block type to its CRAM address,
# using Quartus's own assembler (asm2). Run under quartus_asm, not quartus_sh:
#
#   quartus_asm -t bitmap.tcl <base.sof> <block_type> <outdir> [uids.txt|-] [bits.txt]
#
# With uids.txt (one uid per line) only those instances are mapped, and with
# bits.txt only those bits ("-" keeps all instances). A few bits for every
# instance is the anchor probe that extrapolate.py uses.
#
# Every (instance, bit) pair gets id = 1 + instance_index * nbits + bit_index.
# For each id bit k, k.1.sof flips the pairs whose id has bit k set and
# k.0.sof the pairs whose id has it clear. bitmap.py XORs each against the
# base and reads a CRAM bit's id back from the 2W images; a bit that is in
# both or neither of a pair belongs to several ids or none.
# Spare-row instances (*_rsp*) are skipped: get_bit on them aborts Quartus
# with "data < data_width". So are bits that is_bit_defined denies (two
# hviowr receiver bits): get_bit on those aborts with "Illegal Usage".
load_package asm2
load_devices
lassign $quartus(args) base btype outdir only onlybits
if {$only eq "-"} { set only "" }
file mkdir $outdir
set h [open_sof $base]
set dev [get_device_name $h]
set bits [get_bit_names $dev $btype]
if {$onlybits ne ""} {
    set fh [open $onlybits]
    set wantbits [lsearch -all -inline -not [split [read $fh] \n] {}]
    close $fh
    set bits [lmap b $bits { expr {[lsearch -exact $wantbits $b] >= 0 ? $b : [continue]} }]
}
set uids {}
set keep {}
if {$only ne ""} {
    set fh [open $only]
    foreach u [split [read $fh] \n] { if {$u ne ""} { dict set keep $u 1 } }
    close $fh
}
foreach u [get_uids $dev $btype] {
    if {[string match *_rsp* $u]} continue
    if {$only ne "" && ![dict exists $keep $u]} continue
    lappend uids $u
}
set nb [llength $bits]
set n [expr {[llength $uids] * $nb}]
set w 1
while {(1 << $w) <= $n} { incr w }
set f [open $outdir/pairs.txt w]
puts $f "# $dev $btype uids=[llength $uids] bits=$nb width=$w"
set vals {}
set id 1
foreach u $uids { foreach b $bits {
    if {[is_bit_defined $h $u $b]} {
        set v [get_bit $h $u $b]
        puts $f "$id $u $b $v"
    } else {
        set v -
    }
    lappend vals $v
    incr id
} }
close $f
close_handle $h
for {set k 0} {$k < $w} {incr k} {
    foreach want {1 0} {
        set h [open_sof $base]
        set id 1
        set i 0
        foreach u $uids { foreach b $bits {
            if {(($id >> $k) & 1) == $want && [lindex $vals $i] ne "-"} { set_bit $h $u $b [expr {![lindex $vals $i]}] }
            incr id; incr i
        } }
        write_sof $h $outdir/$k.$want.sof
        close_handle $h
        puts "bitmap: $btype k=$k want=$want"
    }
}
