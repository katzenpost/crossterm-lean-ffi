/-
SPDX-FileCopyrightText: Copyright (C) 2026 David Stainton
SPDX-License-Identifier: AGPL-3.0-only
-/

import CrosstermFfi

open CrosstermFfi

/-- In the order of `samples` in `rust/src/lib.rs`. -/
def expected : List Event :=
  [.key (.char 'a'), .key (.char 'Z') { shift := true }, .key (.char 'c') { ctrl := true },
   .key (.char 'x') { shift := true, ctrl := true, alt := true }, .key (.char 'é'),
   .key (.char '🦀')] ++
  RawEvent.specialKeys.toList.map (.key ·) ++
  [.key (.f 1), .key (.f 12), .resize 80 24, .paste "hello\nworld"]

def check (name : String) (ok : Bool) : StateT Nat IO Unit := do
  IO.println s!"{if ok then "ok  " else "FAIL"} {name}"
  unless ok do modify (· + 1)

def raw (kind code : UInt32) : RawEvent :=
  { kind, code, modifiers := 0, width := 0, height := 0, text := "" }

def tests : StateT Nat IO Unit := do
  let raws ← sampleEvents
  check "as many samples as expected" (raws.size == expected.length)
  for (r, e) in raws.toList.zip expected do
    check s!"decode {repr e}" (r.decode == some e)
  check "unknown kind" (raw 9 0).decode.isNone
  check "surrogate code" (raw 0 0xD800).decode.isNone
  check "past the special keys" (raw 0 (RawEvent.special + 15)).decode.isNone
  -- Without a terminal there is no size to check, only that the call returns.
  match ← size.toBaseIO with
  | .ok (w, h) => check s!"size {w}x{h}" true
  | .error e => IO.println s!"skip size: {e}"

/-- Print the input events that arrive in raw mode until `q`, or until none came for 3 s. -/
def echoEvents : IO Unit := do
  enableRawMode
  try
    repeat
      match ← poll 3000 with
      | none => break
      | some e =>
        IO.print s!"{repr e}\r\n"
        if e == .key (.char 'q') then break
  finally
    disableRawMode

def main (args : List String) : IO UInt32 := do
  if args == ["echo"] then echoEvents; return 0
  let (_, failed) ← tests.run 0
  IO.println s!"{failed} failed"
  return if failed == 0 then 0 else 1
