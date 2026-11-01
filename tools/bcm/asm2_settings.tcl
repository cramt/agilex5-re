# Settings of a CUSTOM attribute that the Bcm_device dump has no bits for
# (README (q)), recovered through asm2: set each value on one instance of a
# blank image and read the given bits back. Prints bcm_settings.tsv rows.
#   quartus_asm -t asm2_settings.tcl <in.sof> <type> <uid> <attribute path> <intent> <leaf prefix> <bit>...
# <attribute path> is the block-level path asm2 takes; <bit> names are
# relative to <leaf prefix> (the attribute's leaf instance path, "-" for the
# block itself), the way bcm_settings.tsv stores them.
load_package asm2
load_devices
lassign $quartus(args) sof btype uid attr intent prefix
set bits [lrange $quartus(args) 6 end]
set p [expr {$prefix eq "-" ? "" : "$prefix/"}]
set h [open_sof $sof]
set names [join [lmap b $bits {set b "{$b}"}] " "]
foreach s [get_attribute_values [get_device_name $h] $btype $attr] {
    if {[catch {set_attribute $h $uid $attr $s} e]} { puts stderr "$s: $e"; continue }
    set v [lmap b $bits {get_bit $h $uid "$p$b"}]
    puts "$intent\t[file tail $attr]\t$s\t$names\t[join $v { }]"
}
