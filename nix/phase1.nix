# phase1/: the DE25-Nano's phase 1, its shell lines and HPS IO hash
# (phase1/de25-nano/README.md).
{ lib, runCommand }:

runCommand "agilex5-phase1" {
  src = lib.fileset.toSource {
    root = ../phase1;
    fileset = ../phase1;
  };
} ''
  cp -r $src $out
''
