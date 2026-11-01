// Phase 7 open flow (README (y)): the LAB's control lines and two clock
// domains, each feature on its own LEDs. tick is one CLOCK1_50 cycle in
// 2^25 (every 0.67 s).
//   LED[1:0]  counts ticks while SW0 is up (clock enable from logic);
//   LED[3:2]  counts ticks; KEY1 held clears it at the next tick (an
//             enable that gates a synchronous clear);
//   LED[5:4]  counts on CLOCK2_50 (every 0.67 s, its own prescaler); KEY0
//             held clears it at once (active-low asynchronous clear);
//   LED[6]    follows the prescaler's top bit (toggles every 0.67 s); SW1
//             up holds it off (active-high asynchronous clear);
//   LED[7]    follows the prescaler's top bit delayed; KEY1 held holds it
//             off (synchronous clear).
module ctl (
    input  wire CLOCK1_50, CLOCK2_50,
    input  wire KEY0, KEY1, SW0, SW1,
    output wire [7:0] LED
);
    reg [24:0] pre = 0;
    always @(posedge CLOCK1_50) pre <= pre + 1'b1;
    reg tick = 0;
    always @(posedge CLOCK1_50) tick <= &pre;

    reg [1:0] a = 0;
    always @(posedge CLOCK1_50) if (tick && SW0) a <= a + 1'b1;

    reg [1:0] b = 0;
    always @(posedge CLOCK1_50) if (tick) begin if (!KEY1) b <= 0; else b <= b + 1'b1; end

    reg [24:0] pre2 = 0;
    always @(posedge CLOCK2_50) pre2 <= pre2 + 1'b1;
    reg [1:0] c = 0;
    reg t2 = 0;
    always @(posedge CLOCK2_50) t2 <= &pre2;
    always @(posedge CLOCK2_50 or negedge KEY0) if (!KEY0) c <= 0; else if (t2) c <= c + 1'b1;

    reg d = 0;
    always @(posedge CLOCK1_50 or posedge SW1) if (SW1) d <= 0; else d <= pre[24];

    reg e = 0;
    always @(posedge CLOCK1_50) if (!KEY1) e <= 0; else e <= d;

    assign LED = {e, d, c, b, a};
endmodule
