// -*- mode: verilog; mode: font-lock; indent-tabs-mode: nil -*-
// vi: set et ts=3 sw=3 sts=3:
//
// Copyright 2026 Steffen Persvold
// SPDX-License-Identifier: Apache-2.0
//
// ========================================================================
// File        : hps_wrapper.sv
// Author      : Steffen Persvold
// Created     : April 15, 2026
// ========================================================================
// Description : Agilex 5 HPS subsystem
// ========================================================================
//

module hps_wrapper
  (
    input  wire         clk_sys
   ,input  wire         rst_sys
   // Active high reset from the HPS Reset Manager.
   ,output wire         h2f_reset_reset
   // HPS warm reset request. Identical to watchdog reset except this is asserted by SDM.
   ,output wire         h2f_warm_reset_handshake_reset_req
   // HPS warm reset acknowledge response to SDM. Should be asserted when all HPS soft logic is successfully in reset.
   ,input  wire         h2f_warm_reset_handshake_reset_ack
   // FPGA-to-HPS interrupt requests, lower 32 bits (irq0[x] -> GIC SPI 17+x).
   ,input  wire [31:0]  f2h_irq0
`ifdef ENABLE_FPGA2HPS
   // FPGA2HPS ACE5Lite Slave port (FPGA -> HPS; 256-bit data, 32-bit addr)
   // AW channel
   ,input  wire [4:0]   fpga2hps_awid
   ,input  wire [31:0]  fpga2hps_awaddr
   ,input  wire [3:0]   fpga2hps_awregion
   ,input  wire [1:0]   fpga2hps_awdomain
   ,input  wire [3:0]   fpga2hps_awsnoop
   ,input  wire [7:0]   fpga2hps_awlen
   ,input  wire [2:0]   fpga2hps_awsize
   ,input  wire [2:0]   fpga2hps_arsize
   ,input  wire [1:0]   fpga2hps_awburst
   ,input  wire         fpga2hps_awlock
   ,input  wire [3:0]   fpga2hps_awcache
   ,input  wire [2:0]   fpga2hps_awprot
   ,input  wire [3:0]   fpga2hps_awqos
   ,input  wire [7:0]   fpga2hps_awuser
   ,input  wire [10:0]  fpga2hps_awstashnid
   ,input  wire         fpga2hps_awstashniden
   ,input  wire [4:0]   fpga2hps_awstashlpid
   ,input  wire         fpga2hps_awstashlpiden
   ,input  wire [5:0]   fpga2hps_awatop
   ,input  wire         fpga2hps_awmmusecsid
   ,input  wire [15:0]  fpga2hps_awmmusid
   ,input  wire         fpga2hps_awvalid
   ,output wire         fpga2hps_awready
   // W channel
   ,input  wire [255:0] fpga2hps_wdata
   ,input  wire [31:0]  fpga2hps_wstrb
   ,input  wire         fpga2hps_wlast
   ,input  wire [7:0]   fpga2hps_wuser
   ,input  wire         fpga2hps_wvalid
   ,output wire         fpga2hps_wready
   // B channel
   ,output wire [4:0]   fpga2hps_bid
   ,output wire [1:0]   fpga2hps_bresp
   ,output wire [7:0]   fpga2hps_buser
   ,output wire         fpga2hps_bvalid
   ,input  wire         fpga2hps_bready
   // AR channel
   ,input  wire [4:0]   fpga2hps_arid
   ,input  wire [31:0]  fpga2hps_araddr
   ,input  wire [3:0]   fpga2hps_arregion
   ,input  wire [1:0]   fpga2hps_ardomain
   ,input  wire [3:0]   fpga2hps_arsnoop
   ,input  wire [7:0]   fpga2hps_arlen
   ,input  wire [1:0]   fpga2hps_arburst
   ,input  wire         fpga2hps_arlock
   ,input  wire [3:0]   fpga2hps_arcache
   ,input  wire [2:0]   fpga2hps_arprot
   ,input  wire [3:0]   fpga2hps_arqos
   ,input  wire [7:0]   fpga2hps_aruser
   ,input  wire         fpga2hps_armmusecsid
   ,input  wire [15:0]  fpga2hps_armmusid
   ,input  wire         fpga2hps_arvalid
   ,output wire         fpga2hps_arready
   // R channel
   ,output wire [4:0]   fpga2hps_rid
   ,output wire [255:0] fpga2hps_rdata
   ,output wire [1:0]   fpga2hps_rresp
   ,output wire         fpga2hps_rlast
   ,output wire [7:0]   fpga2hps_ruser
   ,output wire         fpga2hps_rvalid
   ,input  wire         fpga2hps_rready
`endif //  `ifdef ENABLE_FPGA2HPS
`ifdef ENABLE_FPGA2SDRAM
   // FPGA2SDRAM AXI4 Slave port (FPGA -> SDRAM; 256-bit data, 32-bit addr)
   // AW channel
   ,input  wire [4:0]   f2sdram_awid
   ,input  wire [31:0]  f2sdram_awaddr
   ,input  wire [3:0]   f2sdram_awregion
   ,input  wire [7:0]   f2sdram_awlen
   ,input  wire [2:0]   f2sdram_awsize
   ,input  wire [1:0]   f2sdram_awburst
   ,input  wire         f2sdram_awlock
   ,input  wire [3:0]   f2sdram_awcache
   ,input  wire [2:0]   f2sdram_awprot
   ,input  wire [3:0]   f2sdram_awqos
   ,input  wire [24:0]  f2sdram_awuser
   ,input  wire         f2sdram_awvalid
   ,output wire         f2sdram_awready
   // W channel
   ,input  wire [255:0] f2sdram_wdata
   ,input  wire [31:0]  f2sdram_wstrb
   ,input  wire         f2sdram_wlast
   ,input  wire [7:0]   f2sdram_wuser
   ,input  wire         f2sdram_wvalid
   ,output wire         f2sdram_wready
   // B channel
   ,output wire [4:0]   f2sdram_bid
   ,output wire [1:0]   f2sdram_bresp
   ,output wire [7:0]   f2sdram_buser
   ,output wire         f2sdram_bvalid
   ,input  wire         f2sdram_bready
   // AR channel
   ,input  wire [4:0]   f2sdram_arid
   ,input  wire [31:0]  f2sdram_araddr
   ,input  wire [3:0]   f2sdram_arregion
   ,input  wire [7:0]   f2sdram_arlen
   ,input  wire [2:0]   f2sdram_arsize
   ,input  wire [1:0]   f2sdram_arburst
   ,input  wire         f2sdram_arlock
   ,input  wire [3:0]   f2sdram_arcache
   ,input  wire [2:0]   f2sdram_arprot
   ,input  wire [3:0]   f2sdram_arqos
   ,input  wire [24:0]  f2sdram_aruser
   ,input  wire         f2sdram_arvalid
   ,output wire         f2sdram_arready
   // R channel
   ,output wire [4:0]   f2sdram_rid
   ,output wire [255:0] f2sdram_rdata
   ,output wire [1:0]   f2sdram_rresp
   ,output wire         f2sdram_rlast
   ,output wire [7:0]   f2sdram_ruser
   ,output wire         f2sdram_rvalid
   ,input  wire         f2sdram_rready
`endif //  `ifdef ENABLE_FPGA2SDRAM
`ifdef ENABLE_HPS2FPGA
   // HPS2FPGA AXI4 Master port (HPS -> FPGA; 128-bit data, 32-bit addr)
   // AW channel
   ,output wire [3:0]   hps2fpga_awid
   ,output wire [31:0]  hps2fpga_awaddr
   ,output wire [7:0]   hps2fpga_awlen
   ,output wire [2:0]   hps2fpga_awsize
   ,output wire [1:0]   hps2fpga_awburst
   ,output wire         hps2fpga_awlock
   ,output wire [3:0]   hps2fpga_awcache
   ,output wire [2:0]   hps2fpga_awprot
   ,output wire         hps2fpga_awvalid
   ,input  wire         hps2fpga_awready
   // W channel
   ,output wire [127:0] hps2fpga_wdata
   ,output wire [15:0]  hps2fpga_wstrb
   ,output wire         hps2fpga_wlast
   ,output wire         hps2fpga_wvalid
   ,input  wire         hps2fpga_wready
   // B channel
   ,input  wire [3:0]   hps2fpga_bid
   ,input  wire [1:0]   hps2fpga_bresp
   ,input  wire         hps2fpga_bvalid
   ,output wire         hps2fpga_bready
   // AR channel
   ,output wire [3:0]   hps2fpga_arid
   ,output wire [31:0]  hps2fpga_araddr
   ,output wire [7:0]   hps2fpga_arlen
   ,output wire [2:0]   hps2fpga_arsize
   ,output wire [1:0]   hps2fpga_arburst
   ,output wire         hps2fpga_arlock
   ,output wire [3:0]   hps2fpga_arcache
   ,output wire [2:0]   hps2fpga_arprot
   ,output wire         hps2fpga_arvalid
   ,input  wire         hps2fpga_arready
   // R channel
   ,input  wire [3:0]   hps2fpga_rid
   ,input  wire [127:0] hps2fpga_rdata
   ,input  wire [1:0]   hps2fpga_rresp
   ,input  wire         hps2fpga_rlast
   ,input  wire         hps2fpga_rvalid
   ,output wire         hps2fpga_rready
`endif //  `ifdef ENABLE_HPS2FPGA
   // LWHPS2FPGA AXI4 Master port (HPS -> FPGA, lightweight; 32-bit data, 29-bit addr)
   // AW channel
   ,output wire [3:0]   lwhps2fpga_awid
   ,output wire [28:0]  lwhps2fpga_awaddr
   ,output wire [7:0]   lwhps2fpga_awlen
   ,output wire [2:0]   lwhps2fpga_awsize
   ,output wire [1:0]   lwhps2fpga_awburst
   ,output wire         lwhps2fpga_awlock
   ,output wire [3:0]   lwhps2fpga_awcache
   ,output wire [2:0]   lwhps2fpga_awprot
   ,output wire         lwhps2fpga_awvalid
   ,input  wire         lwhps2fpga_awready
   // W channel
   ,output wire [31:0]  lwhps2fpga_wdata
   ,output wire [3:0]   lwhps2fpga_wstrb
   ,output wire         lwhps2fpga_wlast
   ,output wire         lwhps2fpga_wvalid
   ,input  wire         lwhps2fpga_wready
   // B channel
   ,input  wire [3:0]   lwhps2fpga_bid
   ,input  wire [1:0]   lwhps2fpga_bresp
   ,input  wire         lwhps2fpga_bvalid
   ,output wire         lwhps2fpga_bready
   // AR channel
   ,output wire [3:0]   lwhps2fpga_arid
   ,output wire [28:0]  lwhps2fpga_araddr
   ,output wire [7:0]   lwhps2fpga_arlen
   ,output wire [2:0]   lwhps2fpga_arsize
   ,output wire [1:0]   lwhps2fpga_arburst
   ,output wire         lwhps2fpga_arlock
   ,output wire [3:0]   lwhps2fpga_arcache
   ,output wire [2:0]   lwhps2fpga_arprot
   ,output wire         lwhps2fpga_arvalid
   ,input  wire         lwhps2fpga_arready
   // R channel
   ,input  wire [3:0]   lwhps2fpga_rid
   ,input  wire [31:0]  lwhps2fpga_rdata
   ,input  wire [1:0]   lwhps2fpga_rresp
   ,input  wire         lwhps2fpga_rlast
   ,input  wire         lwhps2fpga_rvalid
   ,output wire         lwhps2fpga_rready
   // HPS Peripheral I/O
   ,input  wire         hps_io_hps_osc_clk
   ,output wire         hps_io_mdio0_mdc
   ,inout  wire         hps_io_mdio0_mdio
   ,input  wire         hps_io_emac0_rx_clk
   ,input  wire         hps_io_emac0_rx_ctl
   ,input  wire         hps_io_emac0_rxd0
   ,input  wire         hps_io_emac0_rxd1
   ,input  wire         hps_io_emac0_rxd2
   ,input  wire         hps_io_emac0_rxd3
   ,output wire         hps_io_emac0_tx_clk
   ,output wire         hps_io_emac0_tx_ctl
   ,output wire         hps_io_emac0_txd0
   ,output wire         hps_io_emac0_txd1
   ,output wire         hps_io_emac0_txd2
   ,output wire         hps_io_emac0_txd3
   ,output wire         hps_io_sdmmc_cclk
   ,inout  wire         hps_io_sdmmc_cmd
   ,inout  wire         hps_io_sdmmc_data0
   ,inout  wire         hps_io_sdmmc_data1
   ,inout  wire         hps_io_sdmmc_data2
   ,inout  wire         hps_io_sdmmc_data3
   ,input  wire         hps_io_usb0_clk
   ,inout  wire         hps_io_usb0_data0
   ,inout  wire         hps_io_usb0_data1
   ,inout  wire         hps_io_usb0_data2
   ,inout  wire         hps_io_usb0_data3
   ,inout  wire         hps_io_usb0_data4
   ,inout  wire         hps_io_usb0_data5
   ,inout  wire         hps_io_usb0_data6
   ,inout  wire         hps_io_usb0_data7
   ,input  wire         hps_io_usb0_dir
   ,input  wire         hps_io_usb0_nxt
   ,output wire         hps_io_usb0_stp
   ,output wire         hps_io_uart1_tx
   ,input  wire         hps_io_uart1_rx
   ,inout  wire         hps_io_i2c1_sda
   ,inout  wire         hps_io_i2c1_scl
   ,inout  wire         hps_io_gpio28
   ,inout  wire         hps_io_gpio34
   ,inout  wire         hps_io_gpio40
   ,inout  wire         hps_io_gpio41
   // LPDDR4 I/O
   ,output wire [0:0]   mem_0_cs
   ,output wire [5:0]   mem_0_ca
   ,output wire [0:0]   mem_0_cke
   ,inout  wire [31:0]  mem_0_dq
   ,inout  wire [3:0]   mem_0_dqs_t
   ,inout  wire [3:0]   mem_0_dqs_c
   ,inout  wire [3:0]   mem_0_dmi
   ,output wire [0:0]   mem_0_ck_t
   ,output wire [0:0]   mem_0_ck_c
   ,output wire         mem_0_reset_n
   ,input  wire         oct_rzqin_0
   ,input  wire         ref_clk
   );

   // =========================================================================================
   // HPS misc signals
   // =========================================================================================

   wire [31:0]          fpga2hps_interrupt_irq0_irq = f2h_irq0;   //  fpga2hps_interrupt_irq0.irq,              FPGA-to-HPS interrupts (lower 32 bits).
   wire [31:0]          fpga2hps_interrupt_irq1_irq = '0;   //  fpga2hps_interrupt_irq1.irq,              FPGA-to-HPS interrupts (higher 32 bits).

   // =========================================================================================
   // HPS2EMIF CSR Bridge (AXI4Lite)
   // =========================================================================================
   wire                 io96b0_to_hps_ch0_axil_clk;         //            io96b0_to_hps.ch0_axil_clk,     Clock source signal. Synchronous signals are sampled on the rising edge of this clock.
   wire                 io96b0_to_hps_ch0_axil_reset_n;     //                         .ch0_axil_reset_n, Required placeholder reset. The actual bridge reset is driven by HPS Reset Manager.
   // AW channel
   wire [26:0]          io96b0_to_hps_ch0_axil_awaddr;      //                         .ch0_axil_awaddr,  The address of the first transfer in a write transaction.
   wire [2:0]           io96b0_to_hps_ch0_axil_awprot;      //                         .ch0_axil_awprot,  Placegolder AWPROT. Protection attributes not supported for this bridge.
   wire                 io96b0_to_hps_ch0_axil_awvalid;     //                         .ch0_axil_awvalid, Indicates write address channel signals are valid.
   wire                 io96b0_to_hps_ch0_axil_awready;     //                         .ch0_axil_awready, Indicates a transfer on the write address channel can be accepted.
   // W channel
   wire [31:0]          io96b0_to_hps_ch0_axil_wdata;       //                         .ch0_axil_wdata,   Write data.
   wire [3:0]           io96b0_to_hps_ch0_axil_wstrb;       //                         .ch0_axil_wstrb,   Write strobes indicating which bye lanes hold valid data.
   wire                 io96b0_to_hps_ch0_axil_wvalid;      //                         .ch0_axil_wvalid,  Indicates write data channel signals are valid.
   wire                 io96b0_to_hps_ch0_axil_wready;      //                         .ch0_axil_wready,  Indicates a transfer on the write data channel can be accepted.
   // B channel
   wire [1:0]           io96b0_to_hps_ch0_axil_bresp;       //                         .ch0_axil_bresp,   Indicates status of write transaction.
   wire                 io96b0_to_hps_ch0_axil_bvalid;      //                         .ch0_axil_bvalid,  Indicates write response channel signals are valid.
   wire                 io96b0_to_hps_ch0_axil_bready;      //                         .ch0_axil_bready,  Indicates a transfer on the write response channel can be accepted.
   // AR channel
   wire [26:0]          io96b0_to_hps_ch0_axil_araddr;      //                         .ch0_axil_araddr,  The address of the first transfer in a read transaction.
   wire [2:0]           io96b0_to_hps_ch0_axil_arprot;      //                         .ch0_axil_arprot,  Placeholder ARPROT. Protection attributes not supported for this bridge.
   wire                 io96b0_to_hps_ch0_axil_arvalid;     //                         .ch0_axil_arvalid, Indicates read address channel signals are valid.
   wire                 io96b0_to_hps_ch0_axil_arready;     //                         .ch0_axil_arready, Indicates a transfer on the read address channel can be accepted.
   // R channel
   wire [31:0]          io96b0_to_hps_ch0_axil_rdata;       //                         .ch0_axil_rdata,   Read data.
   wire [1:0]           io96b0_to_hps_ch0_axil_rresp;       //                         .ch0_axil_rresp,   Indicates status of read transaction.
   wire                 io96b0_to_hps_ch0_axil_rvalid;      //                         .ch0_axil_rvalid,  Indicates read data channel signals are valid.
   wire                 io96b0_to_hps_ch0_axil_rready;      //                         .ch0_axil_rready,  Indicates a transfer on the read data channel can be accepted.

   // =========================================================================================
   // HPS2EMIF Bridge (AXI4)
   // =========================================================================================
   wire                 io96b0_to_hps_axi4_ch0_clk;         //                         .axi4_ch0_clk,     Clock source signal. Synchronous signals are sampled on the rising edge of this clock.
   wire                 io96b0_to_hps_axi4_ch0_reset_n;     //                         .axi4_ch0_reset_n, Required placeholder reset. The actual bridge reset is driven by HPS Reset Manager.
   // AW channel
   wire [6:0]           io96b0_to_hps_axi4_ch0_awid;        //                         .axi4_ch0_awid,    Identification tag for a write transaction.
   wire [39:0]          io96b0_to_hps_axi4_ch0_awaddr;      //                         .axi4_ch0_awaddr,  The address of the first transfer in a write transaction.
   wire [2:0]           io96b0_to_hps_axi4_ch0_awprot;      //                         .axi4_ch0_awprot,  Placeholder AWPROT. Protection attributes not supported for this bridge.
   wire [1:0]           io96b0_to_hps_axi4_ch0_awburst;     //                         .axi4_ch0_awburst, Burst type indicating how address changes between each transfer in a write transaction.
   wire [7:0]           io96b0_to_hps_axi4_ch0_awlen;       //                         .axi4_ch0_awlen,   Exact number of data transfers in a write transaction.
   wire                 io96b0_to_hps_axi4_ch0_awlock;      //                         .axi4_ch0_awlock,  Provides info on atomic characteristics of a write transaction.
   wire [3:0]           io96b0_to_hps_axi4_ch0_awqos;       //                         .axi4_ch0_awqos,   Quality of service identifier for a write transaction.
   wire [2:0]           io96b0_to_hps_axi4_ch0_awsize;      //                         .axi4_ch0_awsize,  Number of bytes in each data transfer in a write transaction.
   wire [13:0]          io96b0_to_hps_axi4_ch0_awuser;      //                         .axi4_ch0_awuser,  Extension of write address channel.
   wire                 io96b0_to_hps_axi4_ch0_awvalid;     //                         .axi4_ch0_awvalid, Indicates write address channel signals are valid.
   wire                 io96b0_to_hps_axi4_ch0_awready;     //                         .axi4_ch0_awready, Indicates a transfer on the write address channel can be accepted.
   // W channel
   wire [255:0]         io96b0_to_hps_axi4_ch0_wdata;       //                         .axi4_ch0_wdata,   Write data.
   wire                 io96b0_to_hps_axi4_ch0_wlast;       //                         .axi4_ch0_wlast,   Indicates the last data transfer in a write transaction.
   wire [31:0]          io96b0_to_hps_axi4_ch0_wstrb;       //                         .axi4_ch0_wstrb,   Write strobes indicating which byte lanes hold valid data.
   wire [31:0]          io96b0_to_hps_axi4_ch0_wuser;       //                         .axi4_ch0_wuser,   Extension of the write data channel.
   wire                 io96b0_to_hps_axi4_ch0_wvalid;      //                         .axi4_ch0_wvalid,  Indicates write data channel signals are valid.
   wire                 io96b0_to_hps_axi4_ch0_wready;      //                         .axi4_ch0_wready,  Indicates a transfer on the write data channel can be accepted.
   // B channel
   wire [6:0]           io96b0_to_hps_axi4_ch0_bid;         //                         .axi4_ch0_bid,     Identification tag for a write response.
   wire [1:0]           io96b0_to_hps_axi4_ch0_bresp;       //                         .axi4_ch0_bresp,   Indicates the status of a write transaction.
   wire                 io96b0_to_hps_axi4_ch0_bvalid;      //                         .axi4_ch0_bvalid,  Indicates write response channel signals are valid.
   wire                 io96b0_to_hps_axi4_ch0_bready;      //                         .axi4_ch0_bready,  Indicates a transfer on write response channel can be accepted.
   // AR channel
   wire [6:0]           io96b0_to_hps_axi4_ch0_arid;        //                         .axi4_ch0_arid,    Identification tag for a read transaction.
   wire [39:0]          io96b0_to_hps_axi4_ch0_araddr;      //                         .axi4_ch0_araddr,  The address of the first transfer in a read transaction.
   wire [2:0]           io96b0_to_hps_axi4_ch0_arprot;      //                         .axi4_ch0_arprot,  Placeholder ARPROT. Protection attributes not supported for this bridge.
   wire [1:0]           io96b0_to_hps_axi4_ch0_arburst;     //                         .axi4_ch0_arburst, Burst type indicating how address changes between each transfer in a read transaction.
   wire [7:0]           io96b0_to_hps_axi4_ch0_arlen;       //                         .axi4_ch0_arlen,   Exact number of data transfers in a read transaction.
   wire                 io96b0_to_hps_axi4_ch0_arlock;      //                         .axi4_ch0_arlock,  Provides info on atomic characteristics of a read transaction.
   wire [3:0]           io96b0_to_hps_axi4_ch0_arqos;       //                         .axi4_ch0_arqos,   Quality of service identifier for a read transaction.
   wire [2:0]           io96b0_to_hps_axi4_ch0_arsize;      //                         .axi4_ch0_arsize,  The number of bytes in each data transfer in a read transaction.
   wire [13:0]          io96b0_to_hps_axi4_ch0_aruser;      //                         .axi4_ch0_aruser,  Extension of read address channel.
   wire                 io96b0_to_hps_axi4_ch0_arvalid;     //                         .axi4_ch0_arvalid, Indicates read address channel signals are valid.
   wire                 io96b0_to_hps_axi4_ch0_arready;     //                         .axi4_ch0_arready, Indicates a transfer on the read address channel can be accepted.
   // R channel
   wire [6:0]           io96b0_to_hps_axi4_ch0_rid;         //                         .axi4_ch0_rid,     Identification tag for read data and response.
   wire [255:0]         io96b0_to_hps_axi4_ch0_rdata;       //                         .axi4_ch0_rdata,   Read data.
   wire [1:0]           io96b0_to_hps_axi4_ch0_rresp;       //                         .axi4_ch0_rresp,   Indicates the status of a read transfer.
   wire                 io96b0_to_hps_axi4_ch0_rlast;       //                         .axi4_ch0_rlast,   Indicates the last data transfer in a read transaction.
   wire [31:0]          io96b0_to_hps_axi4_ch0_ruser;       //                         .axi4_ch0_ruser,   Extension of read data channel.
   wire                 io96b0_to_hps_axi4_ch0_rvalid;      //                         .axi4_ch0_rvalid,  Indicates read data channel signals are valid.
   wire                 io96b0_to_hps_axi4_ch0_rready;      //                         .axi4_ch0_rready,  Indicates a transfer on read data channel can be accepted.

   //

   // Hard Processor System (HPS)
   agilex_hps u_agilex_hps
     (.h2f_reset_reset,
      .h2f_warm_reset_handshake_reset_req,
      .h2f_warm_reset_handshake_reset_ack,
      .fpga2hps_interrupt_irq0_irq,
      .fpga2hps_interrupt_irq1_irq,
      //
      .emac0_app_rst_reset_n      (),
`ifdef ENABLE_HPS2FPGA
      // HPS2FPGA AXI4 Bridge
      .hps2fpga_axi_clock_clk     (clk_sys),
      .hps2fpga_axi_reset_reset   (rst_sys),
      .hps2fpga_awid,
      .hps2fpga_awaddr,
      .hps2fpga_awlen,
      .hps2fpga_awsize,
      .hps2fpga_awburst,
      .hps2fpga_awlock,
      .hps2fpga_awcache,
      .hps2fpga_awprot,
      .hps2fpga_awvalid,
      .hps2fpga_awready,
      .hps2fpga_wdata,
      .hps2fpga_wstrb,
      .hps2fpga_wlast,
      .hps2fpga_wvalid,
      .hps2fpga_wready,
      .hps2fpga_bid,
      .hps2fpga_bresp,
      .hps2fpga_bvalid,
      .hps2fpga_bready,
      .hps2fpga_arid,
      .hps2fpga_araddr,
      .hps2fpga_arlen,
      .hps2fpga_arsize,
      .hps2fpga_arburst,
      .hps2fpga_arlock,
      .hps2fpga_arcache,
      .hps2fpga_arprot,
      .hps2fpga_arvalid,
      .hps2fpga_arready,
      .hps2fpga_rid,
      .hps2fpga_rdata,
      .hps2fpga_rresp,
      .hps2fpga_rlast,
      .hps2fpga_rvalid,
      .hps2fpga_rready,
`endif //  `ifdef ENABLE_HPS2FPGA
      // LightWeight AXI4 HPS2FPGA Bridge
      .lwhps2fpga_axi_clock_clk   (clk_sys),
      .lwhps2fpga_axi_reset_reset (rst_sys),
      .lwhps2fpga_awid,
      .lwhps2fpga_awaddr,
      .lwhps2fpga_awlen,
      .lwhps2fpga_awsize,
      .lwhps2fpga_awburst,
      .lwhps2fpga_awlock,
      .lwhps2fpga_awcache,
      .lwhps2fpga_awprot,
      .lwhps2fpga_awvalid,
      .lwhps2fpga_awready,
      .lwhps2fpga_wdata,
      .lwhps2fpga_wstrb,
      .lwhps2fpga_wlast,
      .lwhps2fpga_wvalid,
      .lwhps2fpga_wready,
      .lwhps2fpga_bid,
      .lwhps2fpga_bresp,
      .lwhps2fpga_bvalid,
      .lwhps2fpga_bready,
      .lwhps2fpga_arid,
      .lwhps2fpga_araddr,
      .lwhps2fpga_arlen,
      .lwhps2fpga_arsize,
      .lwhps2fpga_arburst,
      .lwhps2fpga_arlock,
      .lwhps2fpga_arcache,
      .lwhps2fpga_arprot,
      .lwhps2fpga_arvalid,
      .lwhps2fpga_arready,
      .lwhps2fpga_rid,
      .lwhps2fpga_rdata,
      .lwhps2fpga_rresp,
      .lwhps2fpga_rlast,
      .lwhps2fpga_rvalid,
      .lwhps2fpga_rready,
`ifdef ENABLE_FPGA2HPS
      // FPGA2HPS ACE5Lite Bridge
      .fpga2hps_clock_clk         (clk_sys),
      .fpga2hps_reset_reset       (rst_sys),
      .fpga2hps_awid,
      .fpga2hps_awaddr,
      .fpga2hps_awregion,
      .fpga2hps_awdomain,
      .fpga2hps_awsnoop,
      .fpga2hps_awlen,
      .fpga2hps_awsize,
      .fpga2hps_arsize,
      .fpga2hps_awburst,
      .fpga2hps_awlock,
      .fpga2hps_awcache,
      .fpga2hps_awprot,
      .fpga2hps_awqos,
      .fpga2hps_awuser,
      .fpga2hps_awstashnid,
      .fpga2hps_awstashniden,
      .fpga2hps_awstashlpid,
      .fpga2hps_awstashlpiden,
      .fpga2hps_awatop,
      .fpga2hps_awmmusecsid,
      .fpga2hps_awmmusid,
      .fpga2hps_awvalid,
      .fpga2hps_awready,
      .fpga2hps_wdata,
      .fpga2hps_wstrb,
      .fpga2hps_wlast,
      .fpga2hps_wuser,
      .fpga2hps_wvalid,
      .fpga2hps_wready,
      .fpga2hps_bid,
      .fpga2hps_bresp,
      .fpga2hps_buser,
      .fpga2hps_bvalid,
      .fpga2hps_bready,
      .fpga2hps_arid,
      .fpga2hps_araddr,
      .fpga2hps_arregion,
      .fpga2hps_ardomain,
      .fpga2hps_arsnoop,
      .fpga2hps_arlen,
      .fpga2hps_arburst,
      .fpga2hps_arlock,
      .fpga2hps_arcache,
      .fpga2hps_arprot,
      .fpga2hps_arqos,
      .fpga2hps_aruser,
      .fpga2hps_armmusecsid,
      .fpga2hps_armmusid,
      .fpga2hps_arvalid,
      .fpga2hps_arready,
      .fpga2hps_rid,
      .fpga2hps_rdata,
      .fpga2hps_rresp,
      .fpga2hps_rlast,
      .fpga2hps_ruser,
      .fpga2hps_rvalid,
      .fpga2hps_rready,
`endif //  `ifdef ENABLE_FPGA2HPS
`ifdef ENABLE_FPGA2SDRAM
      // FPGA2SDRAM Bridge
      .f2sdram_axi_clock_clk      (clk_sys),
      .f2sdram_axi_reset_reset    (rst_sys),
      .f2sdram_awid,
      .f2sdram_awaddr,
      .f2sdram_awlen,
      .f2sdram_awsize,
      .f2sdram_arsize,
      .f2sdram_awburst,
      .f2sdram_awlock,
      .f2sdram_awcache,
      .f2sdram_awprot,
      .f2sdram_awqos,
      .f2sdram_awregion,
      .f2sdram_awuser,
      .f2sdram_awvalid,
      .f2sdram_awready,
      .f2sdram_wdata,
      .f2sdram_wstrb,
      .f2sdram_wlast,
      .f2sdram_wuser,
      .f2sdram_wvalid,
      .f2sdram_wready,
      .f2sdram_bid,
      .f2sdram_bresp,
      .f2sdram_buser,
      .f2sdram_bvalid,
      .f2sdram_bready,
      .f2sdram_arid,
      .f2sdram_araddr,
      .f2sdram_arlen,
      .f2sdram_arburst,
      .f2sdram_arlock,
      .f2sdram_arcache,
      .f2sdram_arprot,
      .f2sdram_arqos,
      .f2sdram_arregion,
      .f2sdram_aruser,
      .f2sdram_arvalid,
      .f2sdram_arready,
      .f2sdram_rid,
      .f2sdram_rdata,
      .f2sdram_rresp,
      .f2sdram_rlast,
      .f2sdram_ruser,
      .f2sdram_rvalid,
      .f2sdram_rready,
`endif //  `ifdef ENABLE_FPGA2SDRAM
      // HPS Peripheral I/O
      .hps_io_hps_osc_clk,
      .hps_io_mdio0_mdc,
      .hps_io_mdio0_mdio,
      .hps_io_emac0_rx_clk,
      .hps_io_emac0_rx_ctl,
      .hps_io_emac0_rxd0,
      .hps_io_emac0_rxd1,
      .hps_io_emac0_rxd2,
      .hps_io_emac0_rxd3,
      .hps_io_emac0_tx_clk,
      .hps_io_emac0_tx_ctl,
      .hps_io_emac0_txd0,
      .hps_io_emac0_txd1,
      .hps_io_emac0_txd2,
      .hps_io_emac0_txd3,
      .hps_io_sdmmc_cclk,
      .hps_io_sdmmc_cmd,
      .hps_io_sdmmc_data0,
      .hps_io_sdmmc_data1,
      .hps_io_sdmmc_data2,
      .hps_io_sdmmc_data3,
      .hps_io_usb0_clk,
      .hps_io_usb0_data0,
      .hps_io_usb0_data1,
      .hps_io_usb0_data2,
      .hps_io_usb0_data3,
      .hps_io_usb0_data4,
      .hps_io_usb0_data5,
      .hps_io_usb0_data6,
      .hps_io_usb0_data7,
      .hps_io_usb0_dir,
      .hps_io_usb0_nxt,
      .hps_io_usb0_stp,
      .hps_io_uart1_tx,
      .hps_io_uart1_rx,
      .hps_io_i2c1_sda,
      .hps_io_i2c1_scl,
      .hps_io_gpio28,
      .hps_io_gpio34,
      .hps_io_gpio40,
      .hps_io_gpio41,
      // HPS2EMIF CSR Bridge
      .io96b0_to_hps_ch0_axil_clk,
      .io96b0_to_hps_ch0_axil_reset_n,
      .io96b0_to_hps_ch0_axil_awaddr,
      .io96b0_to_hps_ch0_axil_awprot,
      .io96b0_to_hps_ch0_axil_awvalid,
      .io96b0_to_hps_ch0_axil_awready,
      .io96b0_to_hps_ch0_axil_wdata,
      .io96b0_to_hps_ch0_axil_wstrb,
      .io96b0_to_hps_ch0_axil_wvalid,
      .io96b0_to_hps_ch0_axil_wready,
      .io96b0_to_hps_ch0_axil_bresp,
      .io96b0_to_hps_ch0_axil_bvalid,
      .io96b0_to_hps_ch0_axil_bready,
      .io96b0_to_hps_ch0_axil_araddr,
      .io96b0_to_hps_ch0_axil_arprot,
      .io96b0_to_hps_ch0_axil_arvalid,
      .io96b0_to_hps_ch0_axil_arready,
      .io96b0_to_hps_ch0_axil_rdata,
      .io96b0_to_hps_ch0_axil_rresp,
      .io96b0_to_hps_ch0_axil_rvalid,
      .io96b0_to_hps_ch0_axil_rready,
      // HPS2EMIF Bridge
      .io96b0_to_hps_axi4_ch0_clk,
      .io96b0_to_hps_axi4_ch0_reset_n,
      .io96b0_to_hps_axi4_ch0_awaddr,
      .io96b0_to_hps_axi4_ch0_awprot,
      .io96b0_to_hps_axi4_ch0_awburst,
      .io96b0_to_hps_axi4_ch0_awid,
      .io96b0_to_hps_axi4_ch0_awlen,
      .io96b0_to_hps_axi4_ch0_awlock,
      .io96b0_to_hps_axi4_ch0_awqos,
      .io96b0_to_hps_axi4_ch0_awsize,
      .io96b0_to_hps_axi4_ch0_awuser,
      .io96b0_to_hps_axi4_ch0_awvalid,
      .io96b0_to_hps_axi4_ch0_awready,
      .io96b0_to_hps_axi4_ch0_wdata,
      .io96b0_to_hps_axi4_ch0_wlast,
      .io96b0_to_hps_axi4_ch0_wstrb,
      .io96b0_to_hps_axi4_ch0_wuser,
      .io96b0_to_hps_axi4_ch0_wvalid,
      .io96b0_to_hps_axi4_ch0_wready,
      .io96b0_to_hps_axi4_ch0_bid,
      .io96b0_to_hps_axi4_ch0_bresp,
      .io96b0_to_hps_axi4_ch0_bvalid,
      .io96b0_to_hps_axi4_ch0_bready,
      .io96b0_to_hps_axi4_ch0_araddr,
      .io96b0_to_hps_axi4_ch0_arprot,
      .io96b0_to_hps_axi4_ch0_arburst,
      .io96b0_to_hps_axi4_ch0_arid,
      .io96b0_to_hps_axi4_ch0_arlen,
      .io96b0_to_hps_axi4_ch0_arlock,
      .io96b0_to_hps_axi4_ch0_arqos,
      .io96b0_to_hps_axi4_ch0_arsize,
      .io96b0_to_hps_axi4_ch0_aruser,
      .io96b0_to_hps_axi4_ch0_arvalid,
      .io96b0_to_hps_axi4_ch0_arready,
      .io96b0_to_hps_axi4_ch0_rdata,
      .io96b0_to_hps_axi4_ch0_rid,
      .io96b0_to_hps_axi4_ch0_rresp,
      .io96b0_to_hps_axi4_ch0_rlast,
      .io96b0_to_hps_axi4_ch0_ruser,
      .io96b0_to_hps_axi4_ch0_rvalid,
      .io96b0_to_hps_axi4_ch0_rready
      );

   // HPS EMIF Bridge
   emif_io96b_hps u_emif_io96b_hps
     (.s0_noc_axi4lite_clock     (io96b0_to_hps_ch0_axil_clk),
      .s0_noc_axi4lite_reset_n   (io96b0_to_hps_ch0_axil_reset_n),
      .s0_noc_axi4lite_awaddr    (io96b0_to_hps_ch0_axil_awaddr),
      .s0_noc_axi4lite_awprot    (io96b0_to_hps_ch0_axil_awprot),
      .s0_noc_axi4lite_awvalid   (io96b0_to_hps_ch0_axil_awvalid),
      .s0_noc_axi4lite_awready   (io96b0_to_hps_ch0_axil_awready),
      .s0_noc_axi4lite_wdata     (io96b0_to_hps_ch0_axil_wdata),
      .s0_noc_axi4lite_wstrb     (io96b0_to_hps_ch0_axil_wstrb),
      .s0_noc_axi4lite_wvalid    (io96b0_to_hps_ch0_axil_wvalid),
      .s0_noc_axi4lite_wready    (io96b0_to_hps_ch0_axil_wready),
      .s0_noc_axi4lite_bresp     (io96b0_to_hps_ch0_axil_bresp),
      .s0_noc_axi4lite_bvalid    (io96b0_to_hps_ch0_axil_bvalid),
      .s0_noc_axi4lite_bready    (io96b0_to_hps_ch0_axil_bready),
      .s0_noc_axi4lite_araddr    (io96b0_to_hps_ch0_axil_araddr),
      .s0_noc_axi4lite_arprot    (io96b0_to_hps_ch0_axil_arprot),
      .s0_noc_axi4lite_arvalid   (io96b0_to_hps_ch0_axil_arvalid),
      .s0_noc_axi4lite_arready   (io96b0_to_hps_ch0_axil_arready),
      .s0_noc_axi4lite_rresp     (io96b0_to_hps_ch0_axil_rresp),
      .s0_noc_axi4lite_rdata     (io96b0_to_hps_ch0_axil_rdata),
      .s0_noc_axi4lite_rvalid    (io96b0_to_hps_ch0_axil_rvalid),
      .s0_noc_axi4lite_rready    (io96b0_to_hps_ch0_axil_rready),
      //
      .noc_aclk_0                (io96b0_to_hps_axi4_ch0_clk),
      .noc_rst_n_0               (io96b0_to_hps_axi4_ch0_reset_n),
      .s0_axi4_awaddr            (io96b0_to_hps_axi4_ch0_awaddr),
      .s0_axi4_awburst           (io96b0_to_hps_axi4_ch0_awburst),
      .s0_axi4_awid              (io96b0_to_hps_axi4_ch0_awid),
      .s0_axi4_awlen             (io96b0_to_hps_axi4_ch0_awlen),
      .s0_axi4_awlock            (io96b0_to_hps_axi4_ch0_awlock),
      .s0_axi4_awqos             (io96b0_to_hps_axi4_ch0_awqos),
      .s0_axi4_awsize            (io96b0_to_hps_axi4_ch0_awsize),
      .s0_axi4_awvalid           (io96b0_to_hps_axi4_ch0_awvalid),
      .s0_axi4_awuser            (io96b0_to_hps_axi4_ch0_awuser),
      .s0_axi4_awprot            (io96b0_to_hps_axi4_ch0_awprot),
      .s0_axi4_awready           (io96b0_to_hps_axi4_ch0_awready),
      .s0_axi4_wdata             (io96b0_to_hps_axi4_ch0_wdata),
      .s0_axi4_wstrb             (io96b0_to_hps_axi4_ch0_wstrb),
      .s0_axi4_wlast             (io96b0_to_hps_axi4_ch0_wlast),
      .s0_axi4_wuser             (io96b0_to_hps_axi4_ch0_wuser),
      .s0_axi4_wvalid            (io96b0_to_hps_axi4_ch0_wvalid),
      .s0_axi4_wready            (io96b0_to_hps_axi4_ch0_wready),
      .s0_axi4_bid               (io96b0_to_hps_axi4_ch0_bid),
      .s0_axi4_bresp             (io96b0_to_hps_axi4_ch0_bresp),
      .s0_axi4_bvalid            (io96b0_to_hps_axi4_ch0_bvalid),
      .s0_axi4_bready            (io96b0_to_hps_axi4_ch0_bready),
      .s0_axi4_araddr            (io96b0_to_hps_axi4_ch0_araddr),
      .s0_axi4_arburst           (io96b0_to_hps_axi4_ch0_arburst),
      .s0_axi4_arid              (io96b0_to_hps_axi4_ch0_arid),
      .s0_axi4_arlen             (io96b0_to_hps_axi4_ch0_arlen),
      .s0_axi4_arlock            (io96b0_to_hps_axi4_ch0_arlock),
      .s0_axi4_arqos             (io96b0_to_hps_axi4_ch0_arqos),
      .s0_axi4_arsize            (io96b0_to_hps_axi4_ch0_arsize),
      .s0_axi4_arvalid           (io96b0_to_hps_axi4_ch0_arvalid),
      .s0_axi4_aruser            (io96b0_to_hps_axi4_ch0_aruser),
      .s0_axi4_arprot            (io96b0_to_hps_axi4_ch0_arprot),
      .s0_axi4_arready           (io96b0_to_hps_axi4_ch0_arready),
      .s0_axi4_rdata             (io96b0_to_hps_axi4_ch0_rdata),
      .s0_axi4_rid               (io96b0_to_hps_axi4_ch0_rid),
      .s0_axi4_rresp             (io96b0_to_hps_axi4_ch0_rresp),
      .s0_axi4_rlast             (io96b0_to_hps_axi4_ch0_rlast),
      .s0_axi4_ruser             (io96b0_to_hps_axi4_ch0_ruser),
      .s0_axi4_rvalid            (io96b0_to_hps_axi4_ch0_rvalid),
      .s0_axi4_rready            (io96b0_to_hps_axi4_ch0_rready),
      //
      .mem_0_cs,
      .mem_0_ca,
      .mem_0_cke,
      .mem_0_dq,
      .mem_0_dqs_t,
      .mem_0_dqs_c,
      .mem_0_dmi,
      .mem_0_ck_t,
      .mem_0_ck_c,
      .mem_0_reset_n,
      .oct_rzqin_0,
      .ref_clk
      );

endmodule // hps_wrapper
