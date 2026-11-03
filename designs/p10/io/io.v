// Phase 10 (README (z10)): I/O registers from a5pnr, a board self-test.
// KEY0, KEY1 and SW[3:0] are sampled in their pins' input registers, LED[7:0]
// come from output registers, and three GPIO pins exercise what the board
// can't show: an input register's synchronous clear, an asynchronous clear
// on an input register, and an output-enable register.
//   LED[3:0]: SW[3:0] (SW2 sampled once per 1.34 s; SW3 off while KEY0 is held)
//   LED4: blinks, 1.34 s period; LED5: 0.67 s period, frozen while KEY1 is held
//   LED6, LED7: 0.34 s and 0.17 s periods, dark while KEY0 is held
module io (
    input  wire CLOCK1_50,
    input  wire KEY0, KEY1,
    input  wire [3:0] SW,
    input  wire GA, GB,
    output wire [7:0] LED,
    output wire GQ, GZ
);
    wire clk = CLOCK1_50;
    reg k0 = 0, k1 = 0;                       // KEYs are high when up
    always @(posedge clk) begin k0 <= KEY0; k1 <= KEY1; end

    reg [25:0] cnt = 0;
    always @(posedge clk) cnt <= cnt + 1'b1;
    wire tick = &cnt;

    reg [3:0] s = 0;
    always @(posedge clk) s[1:0] <= SW[1:0];
    always @(posedge clk) if (tick) s[2] <= SW[2];
    always @(posedge clk or negedge k0) if (!k0) s[3] <= 0; else s[3] <= SW[3];

    reg [7:0] led = 0;
    always @(posedge clk) led[3:0] <= s;
    always @(posedge clk) led[4] <= cnt[25];
    always @(posedge clk) if (k1) led[5] <= cnt[24];
    wire rst = !k0;
    always @(posedge clk or posedge rst) if (rst) led[6] <= 0; else led[6] <= cnt[23];
    always @(posedge clk or negedge k0) if (!k0) led[7] <= 0; else led[7] <= cnt[22];
    assign LED = led;

    // GPIO: a synchronous clear (KEY1) and an asynchronous clear (KEY0) on
    // input registers, an output register and an OE register.
    reg ga = 0, gb = 0, gq = 0, zq = 0, zo = 0;
    always @(posedge clk) if (!k1) ga <= 0; else ga <= GA;
    always @(posedge clk or posedge rst) if (rst) gb <= 0; else gb <= GB;
    always @(posedge clk) gq <= ga ^ gb ^ cnt[0];
    always @(posedge clk or posedge rst) if (rst) begin zq <= 0; zo <= 0; end else if (k1) begin zq <= cnt[1]; zo <= cnt[2]; end
    assign GQ = gq;
    assign GZ = zo ? zq : 1'bz;
endmodule
