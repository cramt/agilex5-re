// Phase 10 (README (z11)): a multiplier self-test in DSP blocks, after
// chipsmith's examples/mult-selftest: eight products of pseudo-random
// operands from a 64-bit LFSR, one per mode a5pnr maps, each registered in
// its block and checked against a bit-serial shift-and-add reference in the
// fabric (64 cycles per product). The slots run in turn, 128 cycles each;
// after the eighth the pass count steps and the LFSR moves on.
//   LED0: lights on the first wrong product (dark on a good board)
//   LED[3:1]: the pass count's bits 15..13 (LED1 toggles every 0.17 s)
//   LED[7:4]: the number of wrong products, saturating at 15 (dark)
//   0  9 x 9 unsigned        (an 18x18 block)     4  27 x 27 unsigned
//   1  9 x 9 signed                               5  27 x 27 signed
//   2  18 x 18 unsigned                           6  24 x 24 signed (a 27x27 block)
//   3  18 x 18 signed                             7  12 x 16 unsigned (18x18)
module mul (input wire CLOCK1_50, output wire [7:0] LED);
  wire clk = CLOCK1_50;
  // x^64 + x^63 + x^61 + x^60 + 1, maximal.
  reg [63:0] lfsr = 64'h9E3779B97F4A7C15;
  wire [31:0] a = lfsr[31:0], b = lfsr[63:32];
  reg [2:0] mode = 0;
  reg [6:0] step = 0;
  reg [15:0] pass = 0;
  reg [3:0] errors = 0;

  reg [17:0] p0 = 0;
  reg signed [17:0] p1 = 0;
  reg [35:0] p2 = 0;
  reg signed [35:0] p3 = 0;
  reg [53:0] p4 = 0;
  reg signed [53:0] p5 = 0;
  reg signed [47:0] p6 = 0;
  reg [27:0] p7 = 0;
  wire signed [8:0] a1 = a[8:0], b1 = b[8:0];
  wire signed [17:0] a3 = a[17:0], b3 = b[17:0];
  wire signed [26:0] a5 = a[26:0], b5 = {b[31:27], a[31:27], b[16:0]};
  wire signed [23:0] a6 = a[23:0], b6 = b[23:0];
  always @(posedge clk) begin
    p0 <= a[8:0] * b[8:0];
    p1 <= a1 * b1;
    p2 <= a[17:0] * b[17:0];
    p3 <= a3 * b3;
    p4 <= a[26:0] * {b[31:27], a[31:27], b[16:0]};
    p5 <= a5 * b5;
    p6 <= a6 * b6;
    p7 <= a[11:0] * b[15:0];
  end

  // The slot's operands and product, extended to 64 bits with their signs:
  // the low 64 bits of the product agree whatever the signs. (Continuous
  // assignments, so a simulation sees them from time 0.)
  wire [63:0] ra =
      mode == 3'd0 ? a[8:0] : mode == 3'd1 ? {{55{a[8]}}, a[8:0]} :
      mode == 3'd2 ? a[17:0] : mode == 3'd3 ? {{46{a[17]}}, a[17:0]} :
      mode == 3'd4 ? a[26:0] : mode == 3'd5 ? {{37{a5[26]}}, a5} :
      mode == 3'd6 ? {{40{a[23]}}, a[23:0]} : a[11:0];
  wire [63:0] rb =
      mode == 3'd0 ? b[8:0] : mode == 3'd1 ? {{55{b[8]}}, b[8:0]} :
      mode == 3'd2 ? b[17:0] : mode == 3'd3 ? {{46{b[17]}}, b[17:0]} :
      mode == 3'd4 ? {b[31:27], a[31:27], b[16:0]} : mode == 3'd5 ? {{37{b5[26]}}, b5} :
      mode == 3'd6 ? {{40{b[23]}}, b[23:0]} : b[15:0];
  wire [63:0] want =
      mode == 3'd0 ? p0 : mode == 3'd1 ? {{46{p1[17]}}, p1} :
      mode == 3'd2 ? p2 : mode == 3'd3 ? {{28{p3[35]}}, p3} :
      mode == 3'd4 ? p4 : mode == 3'd5 ? {{10{p5[53]}}, p5} :
      mode == 3'd6 ? {{16{p6[47]}}, p6} : p7;

  // The reference: shift and add, one operand bit a cycle, modulo 2^64.
  reg [63:0] ma = 0, mb = 0, acc = 0, want_q = 0;
  wire bad = step == 7'd70 && acc != want_q;
  always @(posedge clk) begin
    step <= step + 1'b1;
    want_q <= want;
    if (step == 0) begin
      ma <= ra;
      mb <= rb;
      acc <= 0;
    end else if (step <= 7'd64) begin
      if (mb[0]) acc <= acc + ma;
      ma <= {ma[62:0], 1'b0};
      mb <= {1'b0, mb[63:1]};
    end
    if (bad && ~&errors) errors <= errors + 1'b1;
    if (&step) begin
      lfsr <= {lfsr[62:0], lfsr[63] ^ lfsr[62] ^ lfsr[60] ^ lfsr[59]};
      mode <= mode + 1'b1;
      if (&mode) pass <= pass + 1'b1;
    end
  end
  assign LED = {errors, pass[15:13], errors != 0};
endmodule
