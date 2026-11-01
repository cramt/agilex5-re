// Phase 5 held-out core design (README (q)): none of its images were used to
// build or tune anything. Different widths, controls and memory shapes from
// designs/ar and designs/mm: a 20-bit up/down counter with enable,
// asynchronous and synchronous clear, a four-input adder, a 16 x 8 MLAB RAM,
// a 2048 x 10 true dual-port M20K with initial contents, and a 9x9 and an
// 18x19 multiply-add in DSP blocks.
module hx (
    input  wire CLOCK2_50,
    input  wire RST_N,
    input  wire CLR_N,
    input  wire EN,
    input  wire UP,
    output wire [4:0] O
);
    wire clk = CLOCK2_50;
    reg [23:0] lfsr = 24'h1;
    always @(posedge clk) lfsr <= {lfsr[22:0], lfsr[23] ^ lfsr[22] ^ lfsr[21] ^ lfsr[16]};

    reg [19:0] cnt;
    always @(posedge clk or negedge RST_N)
        if (!RST_N) cnt <= 20'h5a5a5;
        else if (!CLR_N) cnt <= 0;
        else if (EN) cnt <= UP ? cnt + 1'b1 : cnt - 1'b1;

    reg [13:0] s4;
    always @(posedge clk) s4 <= lfsr[11:0] + lfsr[23:12] + cnt[11:0] + {cnt[19:12], 4'b0};

    (* ramstyle = "MLAB, no_rw_check" *) reg [7:0] lm [0:15];
    reg [7:0] lq;
    always @(posedge clk) begin
        if (lfsr[0]) lm[lfsr[4:1]] <= lfsr[12:5];
        lq <= lm[cnt[3:0]];
    end

    (* ramstyle = "M20K, no_rw_check" *) reg [9:0] tm [0:2047];
    integer k;
    initial for (k = 0; k < 2048; k = k + 1) tm[k] = k[9:0] ^ (k >> 1);
    reg [9:0] qa, qb;
    always @(posedge clk) begin
        if (lfsr[2]) tm[lfsr[22:12]] <= lfsr[9:0];
        qa <= tm[lfsr[22:12]];
    end
    always @(posedge clk) begin
        if (lfsr[3]) tm[cnt[10:0]] <= cnt[19:10];
        qb <= tm[cnt[10:0]];
    end

    reg signed [8:0] a9, b9; reg signed [17:0] a18; reg signed [18:0] b19; reg signed [37:0] ma;
    always @(posedge clk) begin
        a9 <= lfsr[8:0]; b9 <= lfsr[17:9]; a18 <= {lfsr[5:0], cnt[11:0]}; b19 <= {lfsr[23:5]};
        ma <= a9 * b9 + a18 * b19;
    end

    assign O[0] = ^cnt;
    assign O[1] = ^s4;
    assign O[2] = ^lq;
    assign O[3] = ^qa ^ ^qb;
    assign O[4] = ^ma;
endmodule
