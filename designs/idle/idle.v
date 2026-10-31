// Idle-state reference: one toggle flip-flop clocked from CLOCK2_50 (bank 6E)
// driving a pin in the same bank, so the blocks every other design here uses
// (hvio_3_1 with CLOCK1_50 and KEY0, the IO96 banks, the V16 clock path) are
// left at the state Quartus writes for unused blocks (README (n)).
module idle (
    input  wire CLOCK2_50,
    output wire OUT
);
    reg q = 0;
    always @(posedge CLOCK2_50) q <= ~q;
    assign OUT = q;
endmodule
