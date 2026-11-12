# quartus/: the files of a Quartus Prime Pro install the image writer reads,
# in the install's layout (quartus/README.md).
{ lib, runCommand }:

runCommand "agilex5-quartus-subset" {
  src = lib.fileset.toSource {
    root = ../quartus;
    fileset = lib.fileset.difference ../quartus ../quartus/README.md;
  };
} ''
  cp -r $src $out
''
