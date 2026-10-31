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

struct DBCM_DEVICE;
struct DBCM_BUNDLE { const DBCM_DEVICE* get_device() const; };
struct DBCM_BUNDLE_SINGLETON { static DBCM_BUNDLE* get_bundle(); };
struct CUT_DEV_DYGR_DBCM_UTIL { std::string get_blc_rtl_path(DEV_GLOBAL_ID); };
std::shared_ptr<CUT_DEV_DYGR_DBCM_UTIL> cut_dev_obtain_dygr_dbcm_util(const DYGR_DIE_INFO*, const DBCM_DEVICE*);

static const unsigned ROUTER_BASE = 0x80000000u;

static int dump(void*, Tcl_Interp*, int argc, const char** argv) {
  if (argc < 2) { fprintf(stderr, "usage: rg_dump <outdir> [max_nodes]\n"); return 1; }
  std::string dir = argv[1];
  unsigned long max_nodes = argc > 2 ? strtoul(argv[2], 0, 0) : ~0ul;
  DYGR_DIE_INFO* di = DMGR_UTILITY::get_current_device()->get_die_info();
  const DYGR_ROUTE_INFO* ri = di->get_route_info();
  if (!ri) { fprintf(stderr, "rg_dump: route info not loaded (run load_die_info)\n"); return 1; }

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
    // edges.bin: u32 src, u32 nfanout, u32 dst[nfanout], gids minus ROUTER_BASE
    unsigned hdr[2] = {g - ROUTER_BASE, (unsigned)fo.size()};
    fwrite(hdr, 4, 2, fe);
    for (auto& d : fo) { unsigned x = d.v - ROUTER_BASE; fwrite(&x, 4, 1, fe); }
    fprintf(fn, "%u\t%s\t%zu\n", g - ROUTER_BASE, name.c_str(), fo.size());
    auto& pt = per_type[name.substr(0, name.find(':'))];
    pt.first++; pt.second += fo.size();
    e += fo.size();
    if (n % 1000000 == 0)
      fprintf(stderr, "rg_dump: %lu nodes %lu edges %.0f s\n", n, e,
              std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count());
  }
  fclose(fn); fclose(fe);
  FILE* fs = fopen((dir + "/types.tsv").c_str(), "w");
  for (auto& [k, v] : per_type) fprintf(fs, "%s\t%lu\t%lu\n", k.c_str(), v.first, v.second);
  fclose(fs);
  fprintf(stderr, "rg_dump: done, %lu nodes %lu edges\n", n, e);
  return 0;
}

// rg_rtl <gid offset>...: the BCM RTL path of routing elements
static int rtl(void*, Tcl_Interp*, int argc, const char** argv) {
  DYGR_DIE_INFO* di = DMGR_UTILITY::get_current_device()->get_die_info();
  DBCM_BUNDLE* b = DBCM_BUNDLE_SINGLETON::get_bundle();
  fprintf(stderr, "bundle=%p\n", (void*)b);
  const DBCM_DEVICE* dev = b ? b->get_device() : nullptr;
  fprintf(stderr, "dbcm device=%p\n", (void*)dev);
  if (!dev) return 1;
  auto u = cut_dev_obtain_dygr_dbcm_util(di, dev);
  for (int i = 1; i < argc; i++) {
    unsigned g = ROUTER_BASE + strtoul(argv[i], 0, 0);
    std::string name; di->get_element_string_name({g}, name);
    std::vector<DEV_GLOBAL_ID> fi; di->get_fanin({g}, fi);
    fprintf(stderr, "rtl %s %s fanin:", name.c_str(), u->get_blc_rtl_path({g}).c_str());
    for (auto& f : fi) { std::string n; di->get_element_string_name(f, n); fprintf(stderr, " %s", n.c_str()); }
    fprintf(stderr, "\n");
  }
  return 0;
}

extern "C" int Rgdump_Init(Tcl_Interp* in) {
  Tcl_CreateCommand(in, "rg_dump", dump, nullptr, nullptr);
  Tcl_CreateCommand(in, "rg_rtl", rtl, nullptr, nullptr);
  return 0;
}
