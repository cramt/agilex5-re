// chipsmith's picorv32 example (examples/picorv32/src/top.v) on the
// DE25-Nano: PicoRV32 (Claire Wolf, ISC licence, chipsmith's
// examples/picorv32/src/picorv32.v unmodified; RV32I, no counters, mul or
// div) running a four-instruction loop out of a ROM: it counts in x2 and
// stores it to 0x10000000; LED shows that word folded to 8 bits. KEY0
// (active low) resets.
module picorv32_de25(input CLOCK1_50, input KEY0, output [7:0] LED);
  wire clk = CLOCK1_50;
  reg [3:0] rst_cnt = 0;
  wire resetn = &rst_cnt;
  always @(posedge clk)
    if (!KEY0) rst_cnt <= 0;
    else if (!resetn) rst_cnt <= rst_cnt + 1'b1;

  wire mem_valid, mem_instr;
  reg mem_ready = 0;
  wire [31:0] mem_addr, mem_wdata;
  wire [3:0] mem_wstrb;
  reg [31:0] mem_rdata = 0;

  picorv32 #(
    .ENABLE_COUNTERS(0), .ENABLE_COUNTERS64(0), .CATCH_MISALIGN(0), .CATCH_ILLINSN(0),
    .TWO_STAGE_SHIFT(0), .ENABLE_REGS_DUALPORT(1)
  ) cpu (
    .clk(clk), .resetn(resetn), .mem_valid(mem_valid), .mem_instr(mem_instr), .mem_ready(mem_ready),
    .mem_addr(mem_addr), .mem_wdata(mem_wdata), .mem_wstrb(mem_wstrb), .mem_rdata(mem_rdata));

  reg [31:0] rom;
  always @*
    case (mem_addr[4:2])
      0: rom = 32'h100000b7; // lui  x1, 0x10000
      1: rom = 32'h00000113; // li   x2, 0
      2: rom = 32'h00110113; // addi x2, x2, 1
      3: rom = 32'h0020a023; // sw   x2, 0(x1)
      4: rom = 32'hff9ff06f; // j    -8
      default: rom = 32'h00000013;
    endcase

  reg [31:0] leds = 0;
  always @(posedge clk) begin
    mem_ready <= 0;
    if (mem_valid && !mem_ready) begin
      mem_ready <= 1;
      if (mem_addr[28]) begin
        if (|mem_wstrb) leds <= mem_wdata;
      end else
        mem_rdata <= rom;
    end
  end
  assign LED = leds[7:0] ^ leds[15:8] ^ leds[23:16] ^ leds[31:24];
endmodule
