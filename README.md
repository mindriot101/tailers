# Tailers

Multi-tail program capable of tailing:

* [x] log files
* [ ] docker containers
* [ ] kubernetes logs
* [ ] network messages
* ...

The user interface merges all streams into one and prefixes each stream with a
coloured label representing the original source.

## Implementation

The program is written in Rust. It uses threads and channels, but aspires to
transition to the async ecosystem (`tokio`/`async-std`).
