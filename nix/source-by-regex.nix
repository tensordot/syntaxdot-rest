{ lib }:

src: regexes:

let
  isFiltered = src ? _isLibCleanSourceWith;
  origSrc = if isFiltered then src.origSrc else src;
in lib.cleanSourceWith {
  filter = (path: type:
  let relPath = lib.removePrefix (toString origSrc + "/") (toString path);
  in type == "directory" || lib.any (re: builtins.match re relPath != null) regexes);
  inherit src;
}
