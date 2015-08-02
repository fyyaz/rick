# README for Rick

Rick is a Rust INTERCAL interpreter/compiler.

[![Build Status](https://travis-ci.org/birkenfeld/rick.svg?branch=master)](https://travis-ci.org/birkenfeld/rick)

## Credits

Rick is modeled closely on the [C-INTERCAL](http://catb.org/esr/intercal/)
compiler written by Eric S. Raymond and Alex Smith, and some algorithms for
built-in functionality (operators, binary I/O) is adapted from there, as well as
the standard library in `syslib.rs`.

Also, all the sample code from the `code` directory comes from the C-INTERCAL
distribution.  See the `CATALOG` file there for an overview of programs.

The idea of printing fractals while compiling is taken from
[PyPy](http://pypy.org).

## Language

A comprehensive list of INTERCAL resources, including the original manual from
1972, can be found at the C-INTERCAL page.

Rick implements the base INTERCAL-72 language with the following extensions:

* `COME FROM`
* Computed `COME FROM`
* `TRY AGAIN`
* Computed `ABSTAIN`
* Binary array I/O

## The interpreter

The INTERCAL interpreter takes a source file, parses it into an AST (abstract
sadist tree), and interprets the program, going through the statements until
instructed to `GIVE UP`.  This is roughly 10 times slower than a compiled
version.

## The compiler

Rick can also translate the AST to Rust, which is then compiled by the system
Rust compiler.  This is a few orders of magnitude slower than compiling the C
sources generated by C-INTERCAL, but achieves about the same runtime
performance (when compiled with `-O`), while being safe Rust code.

Rick itself uses nightly Rust features, but the generated code is stable-only.

One trick used for sharing code between the interpreter and programs translated
to Rust code is the syntax extension living in `rick_syntex`.  It contains an
attribute that will embed the decorated module's code as a string into the
module at compile time.  This is then written to the generated Rust files while
translating.

## Running

Do `cargo build` as usual.  Then you can `cargo run -- --help` to see the
available options for the compiler.  Basic usage is `cargo run -- input.i` to
generate an executable and `cargo run -- -i input.i` to interpret.

You might want to use the `-b` flag to get rid of an annoying compiler bug (that
is mandated by the INTERCAL handbook).

Optimizations are activated with `-o` (optimizes INTERCAL code, recommended) and
`-O` (makes rustc optimize machine code, not recommended unless you have lots of
time or the program is very small).  There are a few interesting optimizations,
such as folding the entire program to a "print" statement if it does not depend
on any input.

## Testing

The test suite consists of input and output files for the demo programs in
`code`.  Run `python test.py` to run the test suite.  Run `python test.py
--compiled` to also test compiled code, note however that this takes a while.
Use the `--short` flag to skip the most time consuming tests.

## Hacking

I tried to put at least rudimentary comments into the code where it matters.  If
you actually want to delve deeper into the cesspool that is INTERCAL, let me
know what I can do better!
