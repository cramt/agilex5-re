// chipsmith's sha256 example (examples/sha256/src/top.v) on the DE25-Nano:
// Joachim Strömbergson's (secworks) SHA-256 core (BSD-2-Clause, sources in
// chipsmith's examples/sha256/src, unmodified) driven through its register
// interface: a sequencer writes a 512-bit block (a running seed, the word
// index and SW), starts a hash, waits for ready, reads the eight digest
// words and XORs them together; LED shows the fold folded to 8 bits. KEY0
// (active low) resets. Only the board's pins differ from the DE2 version.
module sha256_de25(input CLOCK1_50, input KEY0, input [3:0] SW, output [7:0] LED);
  wire clk = CLOCK1_50;
  reg [1:0] rst_sync = 2'b00;
  always @(posedge clk) rst_sync <= {rst_sync[0], KEY0};
  wire reset_n = rst_sync[1];

  localparam WAIT = 5'd17, POLL = 5'd21, DIGEST = 5'd22, LAST = 5'd29;
  reg [4:0] step = 0;
  reg [31:0] seed = 0;
  reg [31:0] fold = 0;
  reg [31:0] result = 0;

  reg cs, we;
  reg [7:0] address;
  reg [31:0] write_data;
  wire [31:0] read_data;
  wire error;

  always @* begin
    cs = 0; we = 0; address = 0; write_data = 0;
    if (step < 16) begin
      cs = 1; we = 1; address = {4'h1, step[3:0]};
      write_data = seed ^ {step[3:0], 24'b0, SW};
    end else if (step == 16) begin
      cs = 1; we = 1; address = 8'h08; write_data = 32'h5;
    end else if (step == POLL) begin
      cs = 1; address = 8'h09;
    end else if (step >= DIGEST) begin
      cs = 1; address = 8'h20 + (step - DIGEST);
    end
  end

  always @(posedge clk)
    if (!reset_n) begin
      step <= 0; seed <= 0; fold <= 0;
    end else if (step == POLL) begin
      if (read_data[0]) step <= DIGEST;
    end else if (step == LAST) begin
      result <= fold ^ read_data;
      fold <= 0;
      seed <= seed + 1'b1;
      step <= 0;
    end else begin
      if (step >= DIGEST) fold <= fold ^ read_data;
      step <= step + 1'b1;
    end

  sha256 sha (.clk(clk), .reset_n(reset_n), .cs(cs), .we(we), .address(address),
              .write_data(write_data), .read_data(read_data), .error(error));

  assign LED = result[7:0] ^ result[15:8] ^ result[23:16] ^ result[31:24] ^ {7'b0, error};
endmodule
