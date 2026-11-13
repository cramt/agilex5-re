// The smallest HPS-first design for the DE25-Nano: the board's HPS (Terasic
// GHRD settings, via spersvold/de25_nano's agilex_hps.ip and emif_io96b_hps.ip)
// plus a counter. quartus_pfg -o hps=on splits it into a phase-1 .hps.rbf (HPS
// I/O, HPS EMIF, SPL) and a phase-2 .core.rbf that Linux loads (README "HPS-first").
//
// What the HPS can see once the core is in, with no bridge enabled:
//   f2h_irq0[0]  the 0.75 Hz blink (GIC SPI 17, INTID 49), [1] = 1, [2] = 0
//   ([5:3] carry the unused bridge outputs, 0 while the bridges idle)
// and on the lightweight bridge (0x2000_0000) once it is enabled:
//   0x0 reads 0x48505346 ("HPSF"), anything else the 50 MHz counter.
// LEDs are active low: LED0 blinks at 0.75 Hz, LED1 lit, LED2 dark, LED[7:3]
// count up (LED3 0.75 Hz, LED4 0.37 Hz, ...).
// With HPSF_B defined (the second core for the same phase 1): everything twice
// as fast, and [1]/[2] and LED1/LED2 swapped.
module hpsf_top
  (
    input  wire         CLOCK1_50
   ,output wire [7:0]   LED

   ,input  wire         HPS_CLK_25
   ,output wire         HPS_ENET_MDC
   ,inout  wire         HPS_ENET_MDIO
   ,input  wire         HPS_ENET_RX_CLK
   ,input  wire         HPS_ENET_RX_CTL
   ,input  wire [3:0]   HPS_ENET_RX_DATA
   ,output wire         HPS_ENET_TX_CLK
   ,output wire         HPS_ENET_TX_CTL
   ,output wire [3:0]   HPS_ENET_TX_DATA
   ,output wire         HPS_SD_CLK
   ,inout  wire         HPS_SD_CMD
   ,inout  wire [3:0]   HPS_SD_DATA
   ,input  wire         HPS_USB_CLK
   ,inout  wire [7:0]   HPS_USB_DATA
   ,input  wire         HPS_USB_DIR
   ,input  wire         HPS_USB_NXT
   ,output wire         HPS_USB_STP
   ,output wire         HPS_UART_TX
   ,input  wire         HPS_UART_RX
   ,inout  wire         HPS_I2C_SCL
   ,inout  wire         HPS_I2C_SDA
   ,inout  wire         HPS_GSENSOR_I2C_EN
   ,inout  wire         HPS_GSENSOR_INT
   ,inout  wire         HPS_KEY
   ,inout  wire         HPS_LED

   ,input  wire         LPDDR4A_REFCLK_p
   ,output wire         LPDDR4A_CS_n
   ,output wire [5:0]   LPDDR4A_CA
   ,output wire         LPDDR4A_CK
   ,output wire         LPDDR4A_CK_n
   ,output wire         LPDDR4A_CKE
   ,inout  wire [3:0]   LPDDR4A_DM
   ,inout  wire [31:0]  LPDDR4A_DQ
   ,inout  wire [3:0]   LPDDR4A_DQS
   ,inout  wire [3:0]   LPDDR4A_DQS_n
   ,output wire         LPDDR4A_RESET_n
   ,input  wire         LPDDR4A_RZQ
   );

   wire clk = CLOCK1_50;

   reg [29:0] cnt = '0;
   always @(posedge clk) cnt <= cnt + 1'b1;

`ifdef HPSF_B
   wire       blink = cnt[24];
   wire [1:0] marks = 2'b10;
   wire [4:0] count = cnt[28:24];
`else
   wire       blink = cnt[25];
   wire [1:0] marks = 2'b01;
   wire [4:0] count = cnt[29:25];
`endif
   wire [7:0] lit = {count, marks, blink};
   assign LED = ~lit;

   wire h2f_reset_reset;
   wire h2f_warm_reset_handshake_reset_req;
   // Nothing here needs to settle before a warm reset: acknowledge at once.
   wire h2f_warm_reset_handshake_reset_ack = h2f_warm_reset_handshake_reset_req;

   reg [2:0] rst_q = '1;
   always @(posedge clk or posedge h2f_reset_reset)
     if (h2f_reset_reset) rst_q <= '1;
     else rst_q <= {rst_q[1:0], 1'b0};
   wire rst = rst_q[2];

   // Lightweight HPS-to-FPGA bridge: a read-only register file.
   wire [3:0]  awid, arid;
   wire [28:0] awaddr, araddr;
   wire [7:0]  awlen, arlen;
   wire [2:0]  awsize, arsize, awprot, arprot;
   wire [1:0]  awburst, arburst;
   wire        awlock, arlock;
   wire [3:0]  awcache, arcache;
   wire        awvalid, arvalid;
   wire [31:0] wdata;
   wire [3:0]  wstrb;
   wire        wlast, wvalid;
   wire        bready, rready;

   reg         aw_busy = 1'b0, b_pend = 1'b0;
   reg [3:0]   bid = '0;
   reg         r_busy = 1'b0;
   reg [3:0]   rid = '0;
   reg [7:0]   r_left = '0;
   reg [28:0]  r_addr = '0;

   wire awready = !aw_busy && !b_pend;
   wire wready  = aw_busy;
   wire arready = !r_busy;
   wire rvalid  = r_busy;
   wire rlast   = r_busy && r_left == 8'd0;
`ifdef HPSF_LWMAP
   // Mapping build only (tools/hps_routes.tcl; never loaded): every bridge
   // input bit is driven by logic and every output bit read, so Quartus
   // routes all of them (db/sm4revb/hps/lwhps2fpga.tsv).
   reg [31:0] cnt32 = '0;
   always @(posedge clk) cnt32 <= cnt32 + 1'b1;
   wire [31:0] rdata = cnt32 ^ {3'b0, r_addr};
   wire [1:0]  bresp = cnt32[1:0], rresp = cnt32[3:2];
`else
   wire [31:0] rdata = r_addr[11:2] == 10'd0 ? 32'h4850_5346 : cnt[29:0];
   wire [1:0]  bresp = 2'b00, rresp = 2'b00;
`endif

   always @(posedge clk)
     if (rst) begin
        aw_busy <= 1'b0; b_pend <= 1'b0; r_busy <= 1'b0;
     end else begin
        if (awvalid && awready) begin aw_busy <= 1'b1; bid <= awid; end
        if (wvalid && wready && wlast) begin aw_busy <= 1'b0; b_pend <= 1'b1; end
        if (b_pend && bready) b_pend <= 1'b0;
        if (arvalid && arready) begin
           r_busy <= 1'b1; rid <= arid; r_left <= arlen; r_addr <= araddr;
        end else if (rvalid && rready) begin
           if (rlast) r_busy <= 1'b0;
           else begin r_left <= r_left - 1'b1; r_addr <= r_addr + 29'd4; end
        end
     end


   // Quartus refuses the HPS atom if an unused bridge output is left unread
   // (Error 129015): fold them into two spare FPGA-to-HPS IRQ lines.
  wire f2sdram_awready;
  wire f2sdram_wready;
  wire [4:0] f2sdram_bid;
  wire [1:0] f2sdram_bresp;
  wire [7:0] f2sdram_buser;
  wire f2sdram_bvalid;
  wire f2sdram_arready;
  wire [4:0] f2sdram_rid;
  wire [255:0] f2sdram_rdata;
  wire [1:0] f2sdram_rresp;
  wire f2sdram_rlast;
  wire [7:0] f2sdram_ruser;
  wire f2sdram_rvalid;
  wire [3:0] hps2fpga_awid;
  wire [31:0] hps2fpga_awaddr;
  wire [7:0] hps2fpga_awlen;
  wire [2:0] hps2fpga_awsize;
  wire [1:0] hps2fpga_awburst;
  wire hps2fpga_awlock;
  wire [3:0] hps2fpga_awcache;
  wire [2:0] hps2fpga_awprot;
  wire hps2fpga_awvalid;
  wire [127:0] hps2fpga_wdata;
  wire [15:0] hps2fpga_wstrb;
  wire hps2fpga_wlast;
  wire hps2fpga_wvalid;
  wire hps2fpga_bready;
  wire [3:0] hps2fpga_arid;
  wire [31:0] hps2fpga_araddr;
  wire [7:0] hps2fpga_arlen;
  wire [2:0] hps2fpga_arsize;
  wire [1:0] hps2fpga_arburst;
  wire hps2fpga_arlock;
  wire [3:0] hps2fpga_arcache;
  wire [2:0] hps2fpga_arprot;
  wire hps2fpga_arvalid;
  wire hps2fpga_rready;
   wire f2sdram_sink  = ^{f2sdram_awready, f2sdram_wready, f2sdram_bid, f2sdram_bresp, f2sdram_buser, f2sdram_bvalid, f2sdram_arready, f2sdram_rid, f2sdram_rdata, f2sdram_rresp, f2sdram_rlast, f2sdram_ruser, f2sdram_rvalid};
   wire lw_sink = ^{awaddr, awlen, awsize, awburst, awlock, awcache, awprot, wdata, wstrb,
                    arsize, arburst, arlock, arcache, arprot};
   wire hps2fpga_sink = ^{hps2fpga_awid, hps2fpga_awaddr, hps2fpga_awlen, hps2fpga_awsize, hps2fpga_awburst, hps2fpga_awlock, hps2fpga_awcache, hps2fpga_awprot, hps2fpga_awvalid, hps2fpga_wdata, hps2fpga_wstrb, hps2fpga_wlast, hps2fpga_wvalid, hps2fpga_bready, hps2fpga_arid, hps2fpga_araddr, hps2fpga_arlen, hps2fpga_arsize, hps2fpga_arburst, hps2fpga_arlock, hps2fpga_arcache, hps2fpga_arprot, hps2fpga_arvalid, hps2fpga_rready};

   hps_wrapper u_hps
     (.clk_sys                            (clk),
      .rst_sys                            (rst),
      .h2f_reset_reset,
      .h2f_warm_reset_handshake_reset_req,
      .h2f_warm_reset_handshake_reset_ack,
      .f2h_irq0                           ({26'd0, lw_sink, hps2fpga_sink, f2sdram_sink, marks, blink}),
      // The GHRD's HPS has the F2SDRAM and HPS2FPGA bridges; unused here.
      .f2sdram_awid            ('0),
      .f2sdram_awaddr          ('0),
      .f2sdram_awregion        ('0),
      .f2sdram_awlen           ('0),
      .f2sdram_awsize          ('0),
      .f2sdram_awburst         ('0),
      .f2sdram_awlock          ('0),
      .f2sdram_awcache         ('0),
      .f2sdram_awprot          ('0),
      .f2sdram_awqos           ('0),
      .f2sdram_awuser          ('0),
      .f2sdram_awvalid         ('0),
      .f2sdram_awready         (f2sdram_awready),
      .f2sdram_wdata           ('0),
      .f2sdram_wstrb           ('0),
      .f2sdram_wlast           ('0),
      .f2sdram_wuser           ('0),
      .f2sdram_wvalid          ('0),
      .f2sdram_wready          (f2sdram_wready),
      .f2sdram_bid             (f2sdram_bid),
      .f2sdram_bresp           (f2sdram_bresp),
      .f2sdram_buser           (f2sdram_buser),
      .f2sdram_bvalid          (f2sdram_bvalid),
      .f2sdram_bready          (1'b1),
      .f2sdram_arid            ('0),
      .f2sdram_araddr          ('0),
      .f2sdram_arregion        ('0),
      .f2sdram_arlen           ('0),
      .f2sdram_arsize          ('0),
      .f2sdram_arburst         ('0),
      .f2sdram_arlock          ('0),
      .f2sdram_arcache         ('0),
      .f2sdram_arprot          ('0),
      .f2sdram_arqos           ('0),
      .f2sdram_aruser          ('0),
      .f2sdram_arvalid         ('0),
      .f2sdram_arready         (f2sdram_arready),
      .f2sdram_rid             (f2sdram_rid),
      .f2sdram_rdata           (f2sdram_rdata),
      .f2sdram_rresp           (f2sdram_rresp),
      .f2sdram_rlast           (f2sdram_rlast),
      .f2sdram_ruser           (f2sdram_ruser),
      .f2sdram_rvalid          (f2sdram_rvalid),
      .f2sdram_rready          (1'b1),
      .hps2fpga_awid           (hps2fpga_awid),
      .hps2fpga_awaddr         (hps2fpga_awaddr),
      .hps2fpga_awlen          (hps2fpga_awlen),
      .hps2fpga_awsize         (hps2fpga_awsize),
      .hps2fpga_awburst        (hps2fpga_awburst),
      .hps2fpga_awlock         (hps2fpga_awlock),
      .hps2fpga_awcache        (hps2fpga_awcache),
      .hps2fpga_awprot         (hps2fpga_awprot),
      .hps2fpga_awvalid        (hps2fpga_awvalid),
      .hps2fpga_awready        (1'b1),
      .hps2fpga_wdata          (hps2fpga_wdata),
      .hps2fpga_wstrb          (hps2fpga_wstrb),
      .hps2fpga_wlast          (hps2fpga_wlast),
      .hps2fpga_wvalid         (hps2fpga_wvalid),
      .hps2fpga_wready         (1'b1),
      .hps2fpga_bid            ('0),
      .hps2fpga_bresp          ('0),
      .hps2fpga_bvalid         ('0),
      .hps2fpga_bready         (hps2fpga_bready),
      .hps2fpga_arid           (hps2fpga_arid),
      .hps2fpga_araddr         (hps2fpga_araddr),
      .hps2fpga_arlen          (hps2fpga_arlen),
      .hps2fpga_arsize         (hps2fpga_arsize),
      .hps2fpga_arburst        (hps2fpga_arburst),
      .hps2fpga_arlock         (hps2fpga_arlock),
      .hps2fpga_arcache        (hps2fpga_arcache),
      .hps2fpga_arprot         (hps2fpga_arprot),
      .hps2fpga_arvalid        (hps2fpga_arvalid),
      .hps2fpga_arready        (1'b1),
      .hps2fpga_rid            ('0),
      .hps2fpga_rdata          ('0),
      .hps2fpga_rresp          ('0),
      .hps2fpga_rlast          ('0),
      .hps2fpga_rvalid         ('0),
      .hps2fpga_rready         (hps2fpga_rready),

      .lwhps2fpga_awid    (awid),    .lwhps2fpga_awaddr  (awaddr),  .lwhps2fpga_awlen   (awlen),
      .lwhps2fpga_awsize  (awsize),  .lwhps2fpga_awburst (awburst), .lwhps2fpga_awlock  (awlock),
      .lwhps2fpga_awcache (awcache), .lwhps2fpga_awprot  (awprot),  .lwhps2fpga_awvalid (awvalid),
      .lwhps2fpga_awready (awready),
      .lwhps2fpga_wdata   (wdata),   .lwhps2fpga_wstrb   (wstrb),   .lwhps2fpga_wlast   (wlast),
      .lwhps2fpga_wvalid  (wvalid),  .lwhps2fpga_wready  (wready),
      .lwhps2fpga_bid     (bid),     .lwhps2fpga_bresp   (bresp),   .lwhps2fpga_bvalid  (b_pend),
      .lwhps2fpga_bready  (bready),
      .lwhps2fpga_arid    (arid),    .lwhps2fpga_araddr  (araddr),  .lwhps2fpga_arlen   (arlen),
      .lwhps2fpga_arsize  (arsize),  .lwhps2fpga_arburst (arburst), .lwhps2fpga_arlock  (arlock),
      .lwhps2fpga_arcache (arcache), .lwhps2fpga_arprot  (arprot),  .lwhps2fpga_arvalid (arvalid),
      .lwhps2fpga_arready (arready),
      .lwhps2fpga_rid     (rid),     .lwhps2fpga_rdata   (rdata),   .lwhps2fpga_rresp   (rresp),
      .lwhps2fpga_rlast   (rlast),   .lwhps2fpga_rvalid  (rvalid),  .lwhps2fpga_rready  (rready),
      .hps_io_hps_osc_clk  (HPS_CLK_25),
      .hps_io_mdio0_mdc    (HPS_ENET_MDC),
      .hps_io_mdio0_mdio   (HPS_ENET_MDIO),
      .hps_io_emac0_rx_clk (HPS_ENET_RX_CLK),
      .hps_io_emac0_rx_ctl (HPS_ENET_RX_CTL),
      .hps_io_emac0_rxd0   (HPS_ENET_RX_DATA[0]),
      .hps_io_emac0_rxd1   (HPS_ENET_RX_DATA[1]),
      .hps_io_emac0_rxd2   (HPS_ENET_RX_DATA[2]),
      .hps_io_emac0_rxd3   (HPS_ENET_RX_DATA[3]),
      .hps_io_emac0_tx_clk (HPS_ENET_TX_CLK),
      .hps_io_emac0_tx_ctl (HPS_ENET_TX_CTL),
      .hps_io_emac0_txd0   (HPS_ENET_TX_DATA[0]),
      .hps_io_emac0_txd1   (HPS_ENET_TX_DATA[1]),
      .hps_io_emac0_txd2   (HPS_ENET_TX_DATA[2]),
      .hps_io_emac0_txd3   (HPS_ENET_TX_DATA[3]),
      .hps_io_sdmmc_cclk   (HPS_SD_CLK),
      .hps_io_sdmmc_cmd    (HPS_SD_CMD),
      .hps_io_sdmmc_data0  (HPS_SD_DATA[0]),
      .hps_io_sdmmc_data1  (HPS_SD_DATA[1]),
      .hps_io_sdmmc_data2  (HPS_SD_DATA[2]),
      .hps_io_sdmmc_data3  (HPS_SD_DATA[3]),
      .hps_io_usb0_clk     (HPS_USB_CLK),
      .hps_io_usb0_data0   (HPS_USB_DATA[0]),
      .hps_io_usb0_data1   (HPS_USB_DATA[1]),
      .hps_io_usb0_data2   (HPS_USB_DATA[2]),
      .hps_io_usb0_data3   (HPS_USB_DATA[3]),
      .hps_io_usb0_data4   (HPS_USB_DATA[4]),
      .hps_io_usb0_data5   (HPS_USB_DATA[5]),
      .hps_io_usb0_data6   (HPS_USB_DATA[6]),
      .hps_io_usb0_data7   (HPS_USB_DATA[7]),
      .hps_io_usb0_dir     (HPS_USB_DIR),
      .hps_io_usb0_nxt     (HPS_USB_NXT),
      .hps_io_usb0_stp     (HPS_USB_STP),
      .hps_io_uart1_tx     (HPS_UART_TX),
      .hps_io_uart1_rx     (HPS_UART_RX),
      .hps_io_i2c1_scl     (HPS_I2C_SCL),
      .hps_io_i2c1_sda     (HPS_I2C_SDA),
      .hps_io_gpio28       (HPS_GSENSOR_INT),
      .hps_io_gpio34       (HPS_GSENSOR_I2C_EN),
      .hps_io_gpio40       (HPS_KEY),
      .hps_io_gpio41       (HPS_LED),
      .mem_0_cs            (LPDDR4A_CS_n),
      .mem_0_ca            (LPDDR4A_CA),
      .mem_0_ck_t          (LPDDR4A_CK),
      .mem_0_ck_c          (LPDDR4A_CK_n),
      .mem_0_cke           (LPDDR4A_CKE),
      .mem_0_dmi           (LPDDR4A_DM),
      .mem_0_dq            (LPDDR4A_DQ),
      .mem_0_dqs_t         (LPDDR4A_DQS),
      .mem_0_dqs_c         (LPDDR4A_DQS_n),
      .mem_0_reset_n       (LPDDR4A_RESET_n),
      .oct_rzqin_0         (LPDDR4A_RZQ),
      .ref_clk             (LPDDR4A_REFCLK_p));

endmodule
