/-
SPDX-FileCopyrightText: Copyright (C) 2026 David Stainton
SPDX-License-Identifier: AGPL-3.0-only
-/

/-! # crossterm for Lean

Raw mode, the alternate screen, the cursor, the terminal size, mouse capture, focus reporting
and input events, from the Rust crate crossterm. The functions are implemented in `rust/src/lib.rs`. -/

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

inductive MouseButton where
  | left
  | right
  | middle
  deriving DecidableEq, Repr, Inhabited

inductive MouseKind where
  | down (b : MouseButton)
  | up (b : MouseButton)
  /-- Moved with `b` held. -/
  | drag (b : MouseButton)
  /-- Moved with no button held. -/
  | moved
  | scrollDown
  | scrollUp
  | scrollLeft
  | scrollRight
  deriving DecidableEq, Repr, Inhabited

/-- What the mouse did, and where: the column and row, counting from 0. -/
structure MouseEvent where
  kind : MouseKind
  column : Nat
  row : Nat
  modifiers : Modifiers := {}
  deriving DecidableEq, Repr, Inhabited

inductive Event where
  | key (k : Key) (m : Modifiers := {})
  | resize (width height : Nat)
  | paste (text : String)
  /-- Only while mouse capture is on. -/
  | mouse (e : MouseEvent)
  /-- The window gained or lost the focus; only while focus reporting is on. -/
  | focus (gained : Bool)
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
def kindMouse : UInt32 := 3
def kindFocus : UInt32 := 4

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

def buttons : Array MouseButton := #[.left, .right, .middle]

def decodeMouse (code : UInt32) : Option MouseKind :=
  let b := buttons[(code >>> 8).toNat]?
  match code &&& 0xFF with
  | 0 => b.map .down
  | 1 => b.map .up
  | 2 => b.map .drag
  | 3 => some .moved
  | 4 => some .scrollDown
  | 5 => some .scrollUp
  | 6 => some .scrollLeft
  | 7 => some .scrollRight
  | _ => none

def decode (r : RawEvent) : Option Event :=
  if r.kind == kindKey then (decodeKey r.code).map (.key · (decodeModifiers r.modifiers))
  else if r.kind == kindResize then some (.resize r.width.toNat r.height.toNat)
  else if r.kind == kindPaste then some (.paste r.text)
  else if r.kind == kindMouse then
    (decodeMouse r.code).map fun kind =>
      .mouse { kind, column := r.width.toNat, row := r.height.toNat,
               modifiers := decodeModifiers r.modifiers }
  else if r.kind == kindFocus then some (.focus (r.code != 0))
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
@[extern "cl_enable_mouse_capture"] opaque enableMouseCapture : IO Unit
@[extern "cl_disable_mouse_capture"] opaque disableMouseCapture : IO Unit
@[extern "cl_enable_focus_change"] opaque enableFocusChange : IO Unit
@[extern "cl_disable_focus_change"] opaque disableFocusChange : IO Unit

/-- The terminal's width and height, in cells. -/
@[extern "cl_size"] opaque size : IO (UInt16 × UInt16)

@[extern "cl_poll"] opaque pollRaw (timeoutMs : UInt32) : IO (Option RawEvent)

/-- Wait up to `timeoutMs` for an input event; none if none came. -/
def poll (timeoutMs : UInt32) : IO (Option Event) :=
  return (← pollRaw timeoutMs).bind RawEvent.decode

/-- One event of every kind and key, built by the Rust side, to check both sides agree. -/
@[extern "cl_sample_events"] opaque sampleEvents : BaseIO (Array RawEvent)

end CrosstermFfi
