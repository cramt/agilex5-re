// SHA-256 self-test (README, "Self-tests"): secworks' sha256 core (chipsmith's
// examples/sha256/src, unmodified, as designs/p10/bench/sha256_de25.v drives
// it) hashes a fixed one-block message, alternately "abc" and "" (the empty
// message), and each of the eight digest words is compared with the known
// digest (FIPS 180-2's "abc" example; e3b0c442... for ""). A round with every
// word right pulses `round`; any wrong word, or the core's error flag,
// latches FAIL (verdict.v). A round is 30 cycles plus the core's 66.
module sha_st (input wire CLOCK1_50, output wire [7:0] LED);
    localparam MUTATE = 0;   // build.sh sha_st_mut sets 1: digest word 5 of "abc" expected wrong
    wire clk = CLOCK1_50;
    localparam POLL = 5'd21, DIGEST = 5'd22, LAST = 5'd29;
    localparam [255:0] ABC = 256'hba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad,
                       EMPTY = 256'he3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855;
    reg [3:0] rst_cnt = 0;
    wire reset_n = &rst_cnt;
    always @(posedge clk) if (!reset_n) rst_cnt <= rst_cnt + 1'b1;

    reg [4:0] step = 0;
    reg msg = 0;        // 0: "abc", 1: ""
    reg cs, we;
    reg [7:0] address;
    reg [31:0] write_data;
    wire [31:0] read_data;
    wire error;
    // The padded block: the message, a 1 bit, zeros, the length in bits.
    wire [31:0] w0 = msg ? 32'h80000000 : 32'h61626380;
    wire [31:0] w15 = msg ? 32'h0 : 32'h18;
    always @* begin
        cs = 0; we = 0; address = 0; write_data = 0;
        if (step < 16) begin
            cs = 1; we = 1; address = {4'h1, step[3:0]};
            write_data = step == 0 ? w0 : step == 15 ? w15 : 32'h0;
        end else if (step == 16) begin
            cs = 1; we = 1; address = 8'h08; write_data = 32'h5;   // init, SHA-256 mode
        end else if (step == POLL) begin
            cs = 1; address = 8'h09;
        end else if (step >= DIGEST) begin
            cs = 1; address = 8'h20 + (step - DIGEST);
        end
    end

    wire [2:0] k = step - DIGEST;
    wire [255:0] digest = msg ? EMPTY : ABC ^ (MUTATE != 0 ? 256'h1 << (32 * 2) : 256'h0);
    wire [31:0] want = digest[32 * (7 - k) +: 32];
    reg err = 0, round = 0, bad = 0;
    always @(posedge clk) begin
        round <= 1'b0; bad <= 1'b0;
        if (!reset_n) step <= 0;
        else if (step == POLL) begin
            if (read_data[0]) step <= DIGEST;
        end else begin
            if (step >= DIGEST && read_data != want) err <= 1'b1;
            if (step == LAST) begin
                round <= !err && read_data == want && !error;
                bad <= err || read_data != want || error;
                err <= 1'b0; msg <= ~msg; step <= 0;
            end else step <= step + 1'b1;
        end
    end

    sha256 sha (.clk(clk), .reset_n(reset_n), .cs(cs), .we(we), .address(address),
                .write_data(write_data), .read_data(read_data), .error(error));
    verdict vd (.clk(clk), .round(round), .bad(bad), .LED(LED));
endmodule
