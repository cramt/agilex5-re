# The routing graph (graph/, xz-compressed), decompressed into the layout
# `specimens/rg/full` has: what a5pnr and chipsmith's CHIPSMITH_AGILEX5_GRAPH read.
{ lib, runCommand, xz }:

runCommand "agilex5-graph" {
  nativeBuildInputs = [ xz ];
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.fileFilter (f: f.hasExt "xz") ../graph;
  };
} ''
  mkdir $out
  for f in $src/graph/*.xz; do
    xz -dc "$f" > "$out/$(basename "$f" .xz)"
  done
''
