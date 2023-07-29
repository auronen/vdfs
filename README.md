# VDFS tool
Command line tool to create `.vdf` and `.mod` archives for the ZenGin based games made by Piranha Bytes.

At the moment the intended use case is Linux and CI (like GitHub actions).

## Usage

To generate a archive, you can either:  
 - provide a directory (all contents from the directory will be put into the output file.)
``` sh
vdfs -o my_mod.mod path/to/directory
```
 - provide a yaml file with file and directory specification
``` sh
vdfs my_mod.yml
```

To help with usage in scripts, the base path, output file name and comment can be overridden:  

 - `-b` - base path override
 - `-c` - comment override
 - `-o` - output file path override

## The yaml file
A yaml file can be used to describe the contents of a file.

### Example

``` yaml
comment: "This is an example yaml file"
base_dir: "/home/auronen/my/modding/adventure/g1/"
file_path: "/home/auronen/my/modding/adventure/g1/release/"
file_include_globs:
  - "_work/Data/Scripts/_compiled/*.dat"
  - "_work/Data/Scripts/Content/CUTSCENE/OU.BIN"
```

## The vm file
The vm file (used by the original GothicVDFS program made by NicoDE) is a planned feature for the future.

## Features
- [x] archive packing
- [ ] archive unpacking
- [ ] vm files support
- [ ] Union compatible compression
- [ ] file optimization
- [ ] GUI (maybe)

## Sources and acknowledgments  
 - NicoDE - [GothicVDFS](http://www.bendlins.de/nico/gothic2)  
 - @Gratt-5r2 - VDFS Tool, [ZippedStream](https://github.com/Gratt-5r2/ZippedStream) and advice  
 - @lmichaelis - amazing [documentation](https://phoenix.gothickit.dev/engine/formats/vdf)  
