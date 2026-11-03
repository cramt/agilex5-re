// Phase 10 oracle design (README (z10)): I/O registers on HVIO and IO96 pins,
// beyond ct's one input and one output register. Input, output and
// output-enable registers, plain, with a clock enable, an active-high and an
// active-low asynchronous clear, a preset and a synchronous clear, each on its
// own pin so a decoded image shows which control reaches which register. The
// input registers are observed through fabric XORs with an LFSR, the output
// registers are loaded from it.
module iq (
    input  wire CLOCK1_50,
    input  wire EN, ACLR, ACLRN, SCLR,
    input  wire DA, DB, DC, DD, DE, DF,     // HVIO input registers
    input  wire [3:0] SW,                   // IO96 input registers
    output wire [9:0] X,                    // input registers, observed
    output wire QA, QB, QC, QD, QE, QF,     // HVIO output registers
    output wire [3:0] L,                    // IO96 output registers
    output wire ZA, ZB, LZ                  // output-enable registers
);
    wire clk = CLOCK1_50;
    reg [23:0] lfsr = 24'h1;
    always @(posedge clk) lfsr <= {lfsr[22:0], lfsr[23] ^ lfsr[22] ^ lfsr[21] ^ lfsr[16]};

    // Input registers.
    reg ia, ib, ic, id, ie, iF;
    reg [3:0] is;
    always @(posedge clk) ia <= DA;
    always @(posedge clk) if (EN) ib <= DB;
    always @(posedge clk or posedge ACLR) if (ACLR) ic <= 0; else ic <= DC;
    always @(posedge clk or negedge ACLRN) if (!ACLRN) id <= 0; else id <= DD;
    always @(posedge clk or posedge ACLR) if (ACLR) ie <= 1; else ie <= DE;
    always @(posedge clk) if (SCLR) iF <= 0; else iF <= DF;
    always @(posedge clk) is[0] <= SW[0];
    always @(posedge clk) if (EN) is[1] <= SW[1];
    always @(posedge clk or posedge ACLR) if (ACLR) is[2] <= 0; else is[2] <= SW[2];
    always @(posedge clk or posedge ACLR) if (ACLR) is[3] <= 0; else if (EN) is[3] <= SW[3];
    assign X = {is, iF, ie, id, ic, ib, ia} ^ lfsr[9:0];

    // Output registers.
    reg qa, qb, qc, qd, qe, qf;
    reg [3:0] l;
    always @(posedge clk) qa <= lfsr[0];
    always @(posedge clk) if (EN) qb <= lfsr[1];
    always @(posedge clk or posedge ACLR) if (ACLR) qc <= 0; else qc <= lfsr[2];
    always @(posedge clk or negedge ACLRN) if (!ACLRN) qd <= 0; else qd <= lfsr[3];
    always @(posedge clk or posedge ACLR) if (ACLR) qe <= 1; else qe <= lfsr[4];
    always @(posedge clk) if (SCLR) qf <= 0; else qf <= lfsr[5];
    always @(posedge clk) l[0] <= lfsr[6];
    always @(posedge clk) if (EN) l[1] <= lfsr[7];
    always @(posedge clk or posedge ACLR) if (ACLR) l[2] <= 0; else l[2] <= lfsr[8];
    always @(posedge clk or posedge ACLR) if (ACLR) l[3] <= 0; else if (EN) l[3] <= lfsr[9];
    assign {QF, QE, QD, QC, QB, QA} = {qf, qe, qd, qc, qb, qa};
    assign L = l;

    // Output-enable registers, each with its data register.
    reg za, zb, lz, oa, ob, ol;
    always @(posedge clk) begin za <= lfsr[10]; oa <= lfsr[11]; end
    always @(posedge clk or posedge ACLR)
        if (ACLR) begin zb <= 0; ob <= 0; end
        else if (EN) begin zb <= lfsr[12]; ob <= lfsr[13]; end
    always @(posedge clk) begin lz <= lfsr[14]; ol <= lfsr[15]; end
    assign ZA = oa ? za : 1'bz;
    assign ZB = ob ? zb : 1'bz;
    assign LZ = ol ? lz : 1'bz;
endmodule
