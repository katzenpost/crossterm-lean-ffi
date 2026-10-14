# crossterm-lean-ffi

Terminal control for Lean 4 programs, from the Rust crate
[crossterm](https://crates.io/crates/crossterm): raw mode, the alternate screen, the cursor,
the terminal size, mouse capture, focus reporting and input events. The Rust side uses
[lean-sys](https://github.com/katzenpost/lean-sys) for Lean's object model.

## Use

In your `lakefile`:

```toml
[[require]]
name = "crossterm-lean-ffi"
git = "https://github.com/katzenpost/crossterm-lean-ffi"
rev = "<commit>"
```

```lean
import CrosstermFfi
open CrosstermFfi

def main : IO Unit := do
  enableRawMode
  try
    let (w, h) ← size
    IO.print s!"{w}x{h}, press a key\r\n"
    if let some e ← poll 5000 then IO.print s!"{repr e}\r\n"
  finally
    disableRawMode
```

Building needs `cargo` on the `PATH`. Lake builds the crate in `rust/` and links it into your
executables.

## Tests

    cd rust && cargo test     # key and event encoding
    lake exe ffi-test         # Lean decodes every event the Rust side encodes
    scripts/poll-test         # keys typed into a pseudo-terminal arrive as events

## License

AGPL-3.0-only
