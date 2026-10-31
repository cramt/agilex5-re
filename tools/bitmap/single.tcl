# Map bits one at a time (for named bits that share an image bit with other
# named bits, which bitmap.tcl's binary code can't separate). Run under
# quartus_asm:
#   quartus_asm -t single.tcl <base.sof> <uid> <bits.txt> <outdir>
# writes <outdir>/<n>.sof with bit n of bits.txt flipped, and bits.txt with
# the defined bits in order ("n uid bit value").
load_package asm2
load_devices
lassign $quartus(args) base uid bitsfile outdir
file mkdir $outdir
set fh [open $bitsfile]
set bits [lsearch -all -inline -not [split [read $fh] \n] {}]
close $fh
set h [open_sof $base]
set f [open $outdir/pairs.txt w]
set n 0
foreach b $bits {
    if {![is_bit_defined $h $uid $b]} continue
    set v [get_bit $h $uid $b]
    set_bit $h $uid $b [expr {!$v}]
    write_sof $h $outdir/$n.sof
    set_bit $h $uid $b $v
    puts $f "$n $uid $b $v"
    incr n
}
close $f
