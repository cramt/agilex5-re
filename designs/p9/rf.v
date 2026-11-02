// Phase 9 open flow (README (z9)): MLAB RAM from yosys' memory_libmap.
// A 32 x 8 RAM with initial contents and a 16 x 4 RAM (its address bit 4 is
// a constant: the unrouted pin reads 1), each read asynchronously at an
// address that steps every 2^23 cycles (0.17 s) and registered to the LEDs.
//   LED[3:0]: the low half of word a of the 32 x 8 RAM, a = c[27:23];
//             SW0 up writes c[31:24] there, SW0 down leaves the contents.
//   LED[7:4]: word a[3:0] of the 16 x 4 RAM; KEY1 held writes c[31:28].
module rf (
    input  wire CLOCK1_50,
    input  wire KEY1, SW0,
    output reg  [7:0] LED = 0
);
    wire clk = CLOCK1_50;
    reg [31:0] c = 0;
    always @(posedge clk) c <= c + 1'b1;
    wire [4:0] a = c[27:23];

    reg [7:0] m [0:31];
    reg [3:0] n [0:15];
    integer i;
    initial for (i = 0; i < 32; i = i + 1) m[i] = i * 8'd7 + 8'd1;
    initial for (i = 0; i < 16; i = i + 1) n[i] = 4'd15 - i;
    always @(posedge clk) if (SW0) m[a] <= c[31:24];
    always @(posedge clk) if (!KEY1) n[a[3:0]] <= c[31:28];
    always @(posedge clk) LED <= {n[a[3:0]], m[a][3:0]};
endmodule
