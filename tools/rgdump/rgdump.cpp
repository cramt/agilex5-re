// rgdump: dump sm4revb's routing graph by calling Quartus's own DYGR API from
// inside quartus_fit (`load librgdump.so Rgdump`, then `rg_dump <outdir>`).
// The graph is built by Quartus from devdata/10nm/<die>/<die>.udm; going
// through DYGR gives exactly the nodes and edges the fitter routes on.
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <map>
#include <memory>
#include <string>
#include <vector>

extern "C" {
struct Tcl_Interp;
typedef int(Tcl_CmdProc)(void*, Tcl_Interp*, int, const char**);
void* Tcl_CreateCommand(Tcl_Interp*, const char*, Tcl_CmdProc*, void*, void*);
}

// Mirrors of Quartus types, declared only far enough to link against them.
struct DEV_GLOBAL_ID { unsigned v; };
enum DEV_DIE_INFO_TYPE_ENUM { DIE_INFO_TYPE_FIRST_ };
struct DEV_DEVICE;
struct DYGR_DIE_INFO;
struct DYGR_ROUTE_INFO;
struct DMGR_UTILITY { static DEV_DEVICE* get_current_device(); };
struct DEV_DEVICE { DYGR_DIE_INFO* get_die_info() const; };
struct DYGR_DIE_INFO {
  void get_fanout(DEV_GLOBAL_ID, std::vector<DEV_GLOBAL_ID>&) const;
  void get_fanin(DEV_GLOBAL_ID, std::vector<DEV_GLOBAL_ID>&) const;
  bool get_element_string_name(DEV_GLOBAL_ID, std::string&) const;
  DYGR_ROUTE_INFO* get_route_info() const;
};
// DYGR_FORWARD_ITERATOR<DEV_GLOBAL_ID> is a pimpl: one pointer to a
// DYGR_FORWARD_ITERATOR_IMPL whose vtable is
// [2] operator*  [3] operator!=(impl&)  [4] operator++  [5] operator+=  [6] set_to_end
struct ITER_IMPL {
  struct VT {
    void* dtor[2];
    unsigned (*deref)(const ITER_IMPL*);
    bool (*not_equal)(const ITER_IMPL*, const ITER_IMPL*);
    void (*inc)(ITER_IMPL*);
    void (*add)(ITER_IMPL*, unsigned);
    void (*set_to_end)(ITER_IMPL*);
  }* vt;
};
// DYGR_FORWARD_ITERATOR_RANGE: {begin, end}, each a pimpl pointer. The
// iterator constructors are hidden, so go through the exported gids().
// The non-trivial destructor makes it come back through a hidden pointer, as
// Quartus's own class does. The impls are leaked; this runs once per process.
struct GID_RANGE {
  ITER_IMPL* begin;
  ITER_IMPL* end;
  ~GID_RANGE() {}
};
struct DYGR_ROUTE_INFO { GID_RANGE gids() const; };

namespace DYGR_QDL {
// Pimpl handle on one routing edge's configuration, as the assembler sees it.
struct CONFIG_DATA;
struct EDGE_API {
  void* impl;
  void get_mux_config(CONFIG_DATA*);
  unsigned num_sub_mux_configs();
  void get_sub_mux_config(unsigned, CONFIG_DATA*);
  ~EDGE_API();
  bool valid() const;
  std::string get_path();
  std::string get_uid();
};
struct EMULATOR {
  static std::shared_ptr<EMULATOR> obtain(const DEV_DEVICE*, bool);
  EDGE_API get_edge_api(DEV_GLOBAL_ID, DEV_GLOBAL_ID) const;
  std::string get_mode_str() const;
};
}  // namespace DYGR_QDL
struct DYGR_ROUTE_INFO_UDM_BASE { std::string get_string_representation(DEV_GLOBAL_ID) const; };

static const unsigned ROUTER_BASE = 0x80000000u;

// DYGR_QDL::CONFIG_DATA as get_mux_config fills it: the mux instance path
// below the block uid (ileim_col/xleim_ale0/xleim_4) and the mux input
// (muxin[6], stitch). Only these two members are known; the tail is slack.
struct CFG_PROBE {
  unsigned long a = 0;
  std::string mux, input;
  unsigned long rest[32] = {};
};

// Interns strings into ids, written out as one string per line.
struct INTERN {
  std::map<std::string, unsigned> ids;
  std::vector<const std::string*> by_id;
  unsigned get(const std::string& s) {
    auto [it, fresh] = ids.emplace(s, (unsigned)by_id.size());
    if (fresh) by_id.push_back(&it->first);
    return it->second;
  }
  void write(const std::string& path) const {
    FILE* f = fopen(path.c_str(), "w");
    for (auto* s : by_id) fprintf(f, "%s\n", s->c_str());
    fclose(f);
  }
};

// One edge's configuration as "mux input" pairs; hierarchical muxes add
// their sub-mux settings after a ';'. Empty if the edge needs no setting.
static std::string edge_config(DYGR_QDL::EDGE_API& e) {
  CFG_PROBE c;
  e.get_mux_config(reinterpret_cast<DYGR_QDL::CONFIG_DATA*>(&c));
  std::string r = c.mux.empty() && c.input.empty() ? "" : c.mux + " " + c.input;
  unsigned n = e.num_sub_mux_configs();
  for (unsigned k = 0; k < n; k++) {
    CFG_PROBE sc;
    e.get_sub_mux_config(k, reinterpret_cast<DYGR_QDL::CONFIG_DATA*>(&sc));
    r += ";" + sc.mux + " " + sc.input;
  }
  return r;
}

static int dump(void*, Tcl_Interp*, int argc, const char** argv) {
  if (argc < 2) { fprintf(stderr, "usage: rg_dump <outdir> [max_nodes]\n"); return 1; }
  std::string dir = argv[1];
  unsigned long max_nodes = argc > 2 ? strtoul(argv[2], 0, 0) : ~0ul;
  DYGR_DIE_INFO* di = DMGR_UTILITY::get_current_device()->get_die_info();
  const DYGR_ROUTE_INFO* ri = di->get_route_info();
  if (!ri) { fprintf(stderr, "rg_dump: route info not loaded (run load_die_info)\n"); return 1; }

  DEV_DEVICE* dev = DMGR_UTILITY::get_current_device();
  auto em = DYGR_QDL::EMULATOR::obtain(dev, false);
  INTERN uids, cfgs;
  uids.get("");  // id 0: no block / invalid edge
  cfgs.get("");  // id 0: no setting
  unsigned long invalid = 0, configured = 0;
  FILE* fn = fopen((dir + "/nodes.tsv").c_str(), "w");
  FILE* fe = fopen((dir + "/edges.bin").c_str(), "wb");
  GID_RANGE r = ri->gids();
  ITER_IMPL *it = r.begin, *end = r.end;
  std::map<std::string, std::pair<unsigned long, unsigned long>> per_type;  // nodes, fanout edges
  std::vector<DEV_GLOBAL_ID> fo;
  std::string name;
  unsigned long n = 0, e = 0;
  auto t0 = std::chrono::steady_clock::now();
  for (; n < max_nodes && it->vt->not_equal(it, end); it->vt->inc(it), n++) {
    unsigned g = it->vt->deref(it);
    name.clear();
    di->get_element_string_name({g}, name);
    fo.clear();
    di->get_fanout({g}, fo);
    // edges.bin: u32 src, u32 nfanout, then per fanout u32 dst, u32 block
    // uid id (uids.txt), u32 setting id (configs.txt); gids minus ROUTER_BASE
    unsigned hdr[2] = {g - ROUTER_BASE, (unsigned)fo.size()};
    fwrite(hdr, 4, 2, fe);
    for (auto& d : fo) {
      DYGR_QDL::EDGE_API ea = em->get_edge_api({g}, d);
      unsigned rec[3] = {d.v - ROUTER_BASE, 0, 0};
      if (ea.valid()) {
        rec[1] = uids.get(ea.get_uid());
        rec[2] = cfgs.get(edge_config(ea));
        configured += rec[2] != 0;
      } else {
        invalid++;
      }
      fwrite(rec, 4, 3, fe);
    }
    fprintf(fn, "%u\t%s\t%zu\n", g - ROUTER_BASE, name.c_str(), fo.size());
    auto& pt = per_type[name.substr(0, name.find(':'))];
    pt.first++; pt.second += fo.size();
    e += fo.size();
    if (n % 1000000 == 0)
      fprintf(stderr, "rg_dump: %lu nodes %lu edges %.0f s\n", n, e,
              std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count());
  }
  fclose(fn); fclose(fe);
  uids.write(dir + "/uids.txt");
  cfgs.write(dir + "/configs.txt");
  FILE* fs = fopen((dir + "/types.tsv").c_str(), "w");
  for (auto& [k, v] : per_type) fprintf(fs, "%s\t%lu\t%lu\n", k.c_str(), v.first, v.second);
  fclose(fs);
  fprintf(stderr, "rg_dump: done, %lu nodes %lu edges, %lu with a mux setting, %lu with no edge API, %zu blocks, %zu distinct settings\n",
          n, e, configured, invalid, uids.by_id.size() - 1, cfgs.by_id.size() - 1);
  return 0;
}

// rg_rtl <gid offset>...: UDM string form and fanin list of routing elements
static int rtl(void*, Tcl_Interp*, int argc, const char** argv) {
  DYGR_DIE_INFO* di = DMGR_UTILITY::get_current_device()->get_die_info();
  auto* ub = reinterpret_cast<const DYGR_ROUTE_INFO_UDM_BASE*>(di->get_route_info());
  for (int i = 1; i < argc; i++) {
    unsigned g = ROUTER_BASE + strtoul(argv[i], 0, 0);
    std::string name; di->get_element_string_name({g}, name);
    std::vector<DEV_GLOBAL_ID> fi; di->get_fanin({g}, fi);
    fprintf(stderr, "rtl %s %s fanin:", name.c_str(), ub->get_string_representation({g}).c_str());
    for (auto& f : fi) { std::string n; di->get_element_string_name(f, n); fprintf(stderr, " %s", n.c_str()); }
    fprintf(stderr, "\n");
  }
  return 0;
}

// rg_edge <gid offset>...: for each fanin edge of each node, the assembler's
// view of it (block uid, mux path)
static int edge(void*, Tcl_Interp*, int argc, const char** argv) {
  DEV_DEVICE* dev = DMGR_UTILITY::get_current_device();
  DYGR_DIE_INFO* di = dev->get_die_info();
  bool flag = getenv("RG_QDL_FLAG") && atoi(getenv("RG_QDL_FLAG"));
  auto em = DYGR_QDL::EMULATOR::obtain(dev, flag);
  fprintf(stderr, "emulator=%p mode=%s\n", (void*)em.get(), em->get_mode_str().c_str());
  for (int i = 1; i < argc; i++) {
    unsigned g = ROUTER_BASE + strtoul(argv[i], 0, 0);
    std::string name; di->get_element_string_name({g}, name);
    std::vector<DEV_GLOBAL_ID> fi; di->get_fanin({g}, fi);
    for (size_t k = 0; k < fi.size(); k++) {
      std::string sn; di->get_element_string_name(fi[k], sn);
      DYGR_QDL::EDGE_API e = em->get_edge_api(fi[k], {g});
      bool v = e.valid();
      fprintf(stderr, "edge %s <-[%zu] %s valid=%d", name.c_str(), k, sn.c_str(), v);
      fprintf(stderr, " uid=%s path=%s", e.get_uid().c_str(), e.get_path().c_str());
      if (v) {
        fprintf(stderr, " config=[%s]", edge_config(e).c_str());
      }
      fprintf(stderr, "\n");
    }
  }
  return 0;
}

extern "C" int Rgdump_Init(Tcl_Interp* in) {
  Tcl_CreateCommand(in, "rg_dump", dump, nullptr, nullptr);
  Tcl_CreateCommand(in, "rg_rtl", rtl, nullptr, nullptr);
  Tcl_CreateCommand(in, "rg_edge", edge, nullptr, nullptr);
  return 0;
}
