// ECO fuzzing base: four WYSIWYG 6-input LUTs fed by a 6-bit shift register,
// each registered and driven to a 3.3 V HVIO pin. The quartus_fit ECO package
// then changes one LUT mask or location at a time on the post-fit snapshot.
module fz (
    input  wire       CLOCK1_50,
    input  wire       KEY0,
    output wire [3:0] OUT
);
    reg [5:0] s = 0;
    always @(posedge CLOCK1_50) s <= {s[4:0], KEY0};

    wire [3:0] f;
    reg  [3:0] q = 0;
    genvar i;
    generate
        for (i = 0; i < 4; i = i + 1) begin : l
            tennm_lcell_comb #(
                .lut_mask(64'h6996_9669_6996_9669 ^ (64'h1 << i)),
                .dont_touch("on")
            ) lut (
                .dataa(s[0]), .datab(s[1]), .datac(s[2]),
                .datad(s[3]), .datae(s[4]), .dataf(s[5]),
                .combout(f[i])
            );
        end
    endgenerate
    always @(posedge CLOCK1_50) q <= f;
    assign OUT = q;
endmodule
