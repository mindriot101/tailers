# Tailers

Multi-tail program capable of tailing:

* log files
* docker containers
* kubernetes logs
* network messages
* ...

The user interface merges all streams into one and prefixes each stream with a
coloured label representing the original source.

## Implementation

The program is written in Rust using async-std.
