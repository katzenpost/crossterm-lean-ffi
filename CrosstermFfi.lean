/-
SPDX-FileCopyrightText: Copyright (C) 2026 David Stainton
SPDX-License-Identifier: AGPL-3.0-only
-/

/-! # crossterm for Lean

Raw mode, the alternate screen, the cursor, the terminal size and input events, from the Rust
crate crossterm. The functions are implemented in `rust/src/lib.rs`. -/

namespace CrosstermFfi

inductive Key where
  | char (c : Char)
  | enter
  | esc
  | backspace
  | tab
  | backTab
  | delete
  | insert
  | left
  | right
  | up
  | down
  | home
  | end_
  | pageUp
  | pageDown
  /-- The function key Fn. -/
  | f (n : Nat)
  deriving DecidableEq, Repr, Inhabited

structure Modifiers where
  shift : Bool := false
  ctrl : Bool := false
  alt : Bool := false
  deriving DecidableEq, Repr, Inhabited

inductive Event where
  | key (k : Key) (m : Modifiers := {})
  | resize (width height : Nat)
  | paste (text : String)
  deriving DecidableEq, Repr, Inhabited

/-- An event as the Rust side builds it. Its codes are those of `code` in `rust/src/lib.rs`. -/
structure RawEvent where
  kind : UInt32
  code : UInt32
  modifiers : UInt32
  width : UInt32
  height : UInt32
  text : String
  deriving Inhabited, Repr

namespace RawEvent

def kindKey : UInt32 := 0
def kindResize : UInt32 := 1
def kindPaste : UInt32 := 2

/-- Key codes below this are characters. -/
def special : UInt32 := 0x110000
def functionKey : UInt32 := 0x120000

def specialKeys : Array Key :=
  #[.enter, .esc, .backspace, .tab, .backTab, .delete, .insert, .left, .right, .up, .down, .home,
    .end_, .pageUp, .pageDown]

def decodeKey (code : UInt32) : Option Key :=
  if code < special then
    if h : code.toNat.isValidChar then some (.char ⟨code, h⟩) else none
  else if code < functionKey then specialKeys[(code - special).toNat]?
  else some (.f (code - functionKey).toNat)

def decodeModifiers (m : UInt32) : Modifiers :=
  { shift := m &&& 1 != 0, ctrl := m &&& 2 != 0, alt := m &&& 4 != 0 }

def decode (r : RawEvent) : Option Event :=
  if r.kind == kindKey then (decodeKey r.code).map (.key · (decodeModifiers r.modifiers))
  else if r.kind == kindResize then some (.resize r.width.toNat r.height.toNat)
  else if r.kind == kindPaste then some (.paste r.text)
  else none

end RawEvent

@[extern "cl_enable_raw_mode"] opaque enableRawMode : IO Unit
@[extern "cl_disable_raw_mode"] opaque disableRawMode : IO Unit
@[extern "cl_enter_alternate_screen"] opaque enterAlternateScreen : IO Unit
@[extern "cl_leave_alternate_screen"] opaque leaveAlternateScreen : IO Unit
@[extern "cl_show_cursor"] opaque showCursor : IO Unit
@[extern "cl_hide_cursor"] opaque hideCursor : IO Unit
@[extern "cl_enable_bracketed_paste"] opaque enableBracketedPaste : IO Unit
@[extern "cl_disable_bracketed_paste"] opaque disableBracketedPaste : IO Unit

/-- The terminal's width and height, in cells. -/
@[extern "cl_size"] opaque size : IO (UInt16 × UInt16)

@[extern "cl_poll"] opaque pollRaw (timeoutMs : UInt32) : IO (Option RawEvent)

/-- Wait up to `timeoutMs` for an input event; none if none came. -/
def poll (timeoutMs : UInt32) : IO (Option Event) :=
  return (← pollRaw timeoutMs).bind RawEvent.decode

/-- One event of every kind and key, built by the Rust side, to check both sides agree. -/
@[extern "cl_sample_events"] opaque sampleEvents : BaseIO (Array RawEvent)

end CrosstermFfi
