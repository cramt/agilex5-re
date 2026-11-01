// Phase 7 oracle design (README (y)): M20K uses a5pnr needs beyond phase 6's
// always-written 512 x 32 RAM. Block A: 512 x 32 simple dual port with
// initial contents (a different value at every address, so the image shows
// where each content bit is stored), a write enable from a pin and four byte
// enables. Block B: 512 x 32 with a read enable from a pin and a second,
// output register after the read data.
module mq (
    input  wire CLOCK1_50,
    input  wire WE, RE,
    output wire [3:0] O
);
    wire clk = CLOCK1_50;
    reg [31:0] lfsr = 32'h1;
    always @(posedge clk) lfsr <= {lfsr[30:0], lfsr[31] ^ lfsr[21] ^ lfsr[1] ^ lfsr[0]};
    reg [8:0] a = 0;
    always @(posedge clk) a <= a + 1'b1;

    (* ramstyle = "M20K, no_rw_check" *) reg [31:0] ra [0:511];
    integer k;
    initial for (k = 0; k < 512; k = k + 1) ra[k] = (k * 32'h9e3779b1) ^ (k << 21) ^ 32'h5a5a0f0f;
    reg [31:0] qa;
    always @(posedge clk) begin
        if (WE) begin
            if (lfsr[9])  ra[lfsr[8:0]][7:0]   <= lfsr[31:24];
            if (lfsr[10]) ra[lfsr[8:0]][15:8]  <= lfsr[23:16];
            if (lfsr[11]) ra[lfsr[8:0]][23:16] <= lfsr[15:8];
            if (lfsr[12]) ra[lfsr[8:0]][31:24] <= lfsr[7:0];
        end
        qa <= ra[a];
    end

    (* ramstyle = "M20K, no_rw_check" *) reg [31:0] rb [0:511];
    reg [31:0] qb, qb2;
    always @(posedge clk) begin
        if (lfsr[13]) rb[lfsr[22:14]] <= lfsr;
        if (RE) qb <= rb[a];
        qb2 <= qb;
    end

    assign O[0] = ^qa[15:0];
    assign O[1] = ^qa[31:16];
    assign O[2] = ^qb2[15:0];
    assign O[3] = ^qb2[31:16];
endmodule
