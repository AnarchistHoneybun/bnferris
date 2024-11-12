# BNFerris

Generate random messages based on their [BNF](https://en.wikipedia.org/wiki/Backus%E2%80%93Naur_form) (Backus-Naur Form) grammar definitions.

A port of [Tsoding's](https://github.com/rexim) [bnfuzzer](https://github.com/rexim/bnfuzzer).
```console
Usage: bnferris [OPTIONS] --file <FILE> --entry <ENTRY>

Options:
  -f, --file <FILE>    Path to the BNF grammar file
  -e, --entry <ENTRY>  The symbol name to start generating from. Use '!' to list all available symbols
  -c, --count <COUNT>  How many messages to generate [default: 1]
      --verify         Verify that all the symbols are defined
      --unused         Verify that all the symbols are used
      --dump           Dump the text representation of the entry symbol
  -h, --help           Print help
  -V, --version        Print version
```

## Quick Start

Generate 10 random postal addresses:

```console
$ cargo run -- -f ./examples/postal.bnf -e postal-address -c 10
```

## Supported Grammar Syntax

This implementation supports both BNF and ABNF syntaxes, allowing for flexible grammar definitions.
You can mix and match syntax elements from both formats in the same file.

### Core Features

#### Comments

```bnf
; BNF-style comment
// C-style comment
```

#### Rule Definition

```bnf
rule = definition
```

#### Concatenation

```bnf
name = part1 part2 part3
```

#### Alternatives
```bnf
choice = one / two    ; ABNF style
choice = one | two    ; BNF style
```

#### Incremental Alternatives

```bnf
rule = alt1 / alt2
rule =/ alt3
rule =/ alt4 / alt5
```
Equivalent to `rule = alt1 / alt2 / alt3 / alt4 / alt5`

#### Value Ranges

Multiple equivalent syntaxes:
```bnf
digit = %x30-39           ; Hex range
digit = "0" ... "9"       ; Character range
digit = "\x30" ... "\x39" ; Escaped hex range
```

#### Grouping

```bnf
group = item1 (item2 / item3) item4
```

#### Repetition

```bnf
n*mitem    ; Repeat item from n to m times
nitem      ; Exactly n repetitions
*item      ; Zero or more repetitions
```

#### Optional Elements

```bnf
[optional]  ; Zero or one occurrence
```