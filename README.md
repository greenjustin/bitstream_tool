# Bitstream Tool
Simple command line tool designed for manipulating video bitstreams.

Currently only supports H264 Annex B format.

Usage:
```
cargo run -- [-d|-e] <in file> <out file>
```
The `-d` flag will take in an Annex B bitstream and output a human readable,
JSON-like representation of the bitstream headers. The `-e` flag will take a
human readable representation of the bitstream and re-serialize it back into
H264 Annex B.
