import Lake
open Lake DSL System

package «crossterm-lean-ffi» where
  version := v!"0.2.0"
  description := "Terminal control for Lean 4 from the Rust crate crossterm"
  keywords := #["terminal", "tui", "ffi", "rust"]
  license := "AGPL-3.0-only"

@[default_target]
lean_lib CrosstermFfi

lean_exe «ffi-test» where
  root := `CrosstermFfi.Tests

/-- The Rust crate in `rust/`, built by cargo. -/
extern_lib crossterm_lean_ffi pkg := Job.async do
  proc { cmd := "cargo", args := #["build", "--release", "--quiet"], cwd := pkg.dir / "rust" }
  let lib := pkg.dir / "rust" / "target" / "release" / nameToStaticLib "crossterm_lean_ffi"
  setTrace (← computeTrace lib)
  return lib
