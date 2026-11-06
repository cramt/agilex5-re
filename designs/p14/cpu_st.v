// CPU self-test (README, "Self-tests"): FemtoRV32 Quark (designs/p7/cpu,
// unmodified; register file in MLAB under chipsmith's synthesis) running
// cpu_prog.vh (cpu_asm.py) from an M20K whose upper half is its data RAM.
// Each turn of the program's loop computes a checksum over ALU, branch and
// load/store results and stores it to 0x400000; a software check that fails
// stores to 0x400008. The hardware compares each checksum with SUM, the
// value cpu_asm.py's RV32I model computed: a match pulses `round`, anything
// else (or a store to 0x400008) latches FAIL (verdict.v). A turn is about
// 1,100 cycles.
//   0x000-0x3ff  program (M20K, read with the CPU's read strobe)
//   0x400-0x7ff  data (the same M20K, written with the CPU's byte mask)
//   0x400000     store: checksum     0x400008  store: software FAIL
// femtorv32_quark.v's BENCH block zeroes x0, the cycle counter and the shift
// count at power-up, as the flops do on the device; it must come after this
// file.
`define BENCH
module cpu_st (input wire CLOCK1_50, output wire [7:0] LED);
    localparam MUTATE = 0;   // build.sh cpu_st_mut sets 1: the hardware expects SUM ^ 1
`include "cpu_prog.vh"
    wire clk = CLOCK1_50;
    reg [3:0] rst_cnt = 0;
    wire resetn = &rst_cnt;
    always @(posedge clk) if (!resetn) rst_cnt <= rst_cnt + 1'b1;

    wire [31:0] mem_addr, mem_wdata, ram_q;
    wire [3:0] mem_wmask;
    wire mem_rstrb;
    reg io_sel = 0;
    FemtoRV32 core (
        .clk(clk), .mem_addr(mem_addr), .mem_wdata(mem_wdata), .mem_wmask(mem_wmask),
        .mem_rdata(io_sel ? 32'h0 : ram_q), .mem_rstrb(mem_rstrb), .mem_rbusy(1'b0), .mem_wbusy(1'b0),
        .reset(resetn));

    wire io = mem_addr[22];
    A5_M20K #(.INIT(PROG)) ram (
        .CLK(clk), .WE(|mem_wmask && !io), .BE(mem_wmask), .RE(mem_rstrb),
        .WADDR(mem_addr[10:2]), .WDATA(mem_wdata), .RADDR(mem_addr[10:2]), .RDATA(ram_q));

    wire [31:0] want = SUM ^ (MUTATE != 0 ? 32'h1 : 32'h0);
    reg round = 0, bad = 0;
    always @(posedge clk) begin
        round <= 1'b0; bad <= 1'b0;
        if (mem_rstrb) io_sel <= io;
        if (|mem_wmask && io) begin
            if (mem_addr[3]) bad <= 1'b1;
            else begin
                round <= mem_wdata == want;
                bad <= mem_wdata != want;
            end
        end
    end
    verdict vd (.clk(clk), .round(round), .bad(bad), .LED(LED));
endmodule
