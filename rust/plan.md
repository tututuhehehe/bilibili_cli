# Rust migration plan: bilibili_cli (Python)

1 packages, 1 source files (1324 lines), 0 test files, 0 UI files, grouped into 1 units.

| Ready | Caution | Blocked | Keep in source language |
| ---: | ---: | ---: | ---: |
| 0 | 1 | 0 | 0 |

## Start here

Best first ports: safe to port (readiness) and worth porting (loops and string work, tests, size).

| # | Unit | Lines | Priority | Readiness | Tests | Bridge | Main risks |
| ---: | --- | ---: | ---: | ---: | ---: | --- | --- |
| 1 | `bilibili_cli.py` | 1324 | 24 | 15 | 0 | PyO3 | String offsets and slicing (28); Regexes built at runtime (1); Division and modulo (9) |

### `bilibili_cli.py`

1324 lines in 1 files, wave 0, priority 24, readiness 15, bridge **PyO3**.

- Leaf: imports no other unit.
- No tests found: write characterization tests against the original code first.
- String offsets and slicing x28: `bilibili_cli.py:140`, `bilibili_cli.py:183`, `bilibili_cli.py:213`
- Regexes built at runtime x1: `bilibili_cli.py:652`
- Division and modulo x9: `bilibili_cli.py:494`, `bilibili_cli.py:494`, `bilibili_cli.py:494`
- Dynamic typing x8: `bilibili_cli.py:107`, `bilibili_cli.py:146`, `bilibili_cli.py:459`
- Integer width and overflow x4: `bilibili_cli.py:1137`, `bilibili_cli.py:1142`, `bilibili_cli.py:1147`
- Class inheritance x1: `bilibili_cli.py:106`
- Exceptions x49: `bilibili_cli.py:184`, `bilibili_cli.py:188`, `bilibili_cli.py:193`
- Number semantics x17: `bilibili_cli.py:154`, `bilibili_cli.py:155`, `bilibili_cli.py:524`

## Order by package

Packages come after the workspace packages they use. Inside a package, port by wave: a unit's imports are in earlier waves.

### 1. root (`.`, portable, 0 test files)

| Wave | Unit | Status | Lines | Readiness | Bridge | Main risks |
| ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | `bilibili_cli.py` | caution | 1324 | 15 | PyO3 | String offsets and slicing (28); Regexes built at runtime (1) |

## Risk guide

- **String offsets and slicing** (High): JS indexes strings in UTF-16 code units, Rust in UTF-8 bytes. Offsets that cross the boundary must be converted, or non-ASCII text gets wrong ranges silently.
- **Regexes built at runtime** (High): Patterns come from data or config in JS syntax. Rust regex syntax differs, so user-visible behavior can change. Pin it with tests on real patterns.
- **Dynamic typing** (Medium): Values whose shape is checked at runtime need an explicit Rust enum or `serde_json::Value`.
- **Exceptions** (Low): Thrown errors become `Result`. Errors crossing the bridge must map back to the same JS error shape.
- **Class inheritance** (Medium): Class hierarchies become traits plus composition; `instanceof` checks become enums.
- **Number semantics** (Low): JS numbers are f64; bitwise ops truncate to i32; parseInt is lenient. Pick Rust integer types deliberately.
- **Integer width and overflow** (Medium): Python ints never overflow; C, Go and Java ints wrap or are undefined on overflow. Pick i64, i128 or a bignum crate deliberately, and use wrapping_* where the source wraps.
- **Division and modulo** (Medium): Python `//` and `%` floor toward negative infinity; Rust `/` and `%` truncate: use `div_euclid`/`rem_euclid` or explicit floor math. C division by zero is undefined.

## Whole app

Shape: **cli**.

| Entry point | Kind | Found from |
| --- | --- | --- |
| `bilibili_cli.py` | cli | `if __name__ == "__main__"` at bilibili_cli.py:1542 |

### Every dependency

None.

### Pieces, in port order

- wave 0: `bilibili_cli.py` (1324 lines)

### Decisions for the user

**D1** (shape): The app is a cli, entered through bilibili_cli.py. Port every entry point, in that shape?

> Yes: a clap front door (`app front-door`) takes every command over first

