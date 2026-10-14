// SPDX-FileCopyrightText: Copyright (C) 2026 David Stainton
// SPDX-License-Identifier: AGPL-3.0-only

//! crossterm for Lean: raw mode, the alternate screen, the cursor, the terminal size, mouse
//! capture, focus reporting and input events, as functions with Lean's calling convention.
//! `CrosstermFfi.lean` declares them.

#![allow(clippy::missing_safety_doc)]

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use crossterm::{cursor, execute, terminal};
use lean_sys::*;
use std::io::{self, stdout};
use std::time::{Duration, Instant};

/// The codes of `RawEvent`, which `RawEvent.decode` in Lean must match.
pub mod code {
    pub const KIND_KEY: u32 = 0;
    pub const KIND_RESIZE: u32 = 1;
    pub const KIND_PASTE: u32 = 2;
    /// A mouse event: `code` is the kind, and the button above `BUTTON_SHIFT`; `width` and
    /// `height` hold the column and row.
    pub const KIND_MOUSE: u32 = 3;
    /// Focus gained (`code` 1) or lost (`code` 0).
    pub const KIND_FOCUS: u32 = 4;

    pub const MOUSE_DOWN: u32 = 0;
    pub const MOUSE_UP: u32 = 1;
    pub const MOUSE_DRAG: u32 = 2;
    pub const MOUSE_MOVED: u32 = 3;
    pub const SCROLL_DOWN: u32 = 4;
    pub const SCROLL_UP: u32 = 5;
    pub const SCROLL_LEFT: u32 = 6;
    pub const SCROLL_RIGHT: u32 = 7;
    pub const BUTTON_SHIFT: u32 = 8;
    pub const BUTTON_LEFT: u32 = 0;
    pub const BUTTON_RIGHT: u32 = 1;
    pub const BUTTON_MIDDLE: u32 = 2;

    /// Key codes below this are characters.
    pub const SPECIAL: u32 = 0x11_0000;
    pub const ENTER: u32 = SPECIAL;
    pub const ESC: u32 = SPECIAL + 1;
    pub const BACKSPACE: u32 = SPECIAL + 2;
    pub const TAB: u32 = SPECIAL + 3;
    pub const BACK_TAB: u32 = SPECIAL + 4;
    pub const DELETE: u32 = SPECIAL + 5;
    pub const INSERT: u32 = SPECIAL + 6;
    pub const LEFT: u32 = SPECIAL + 7;
    pub const RIGHT: u32 = SPECIAL + 8;
    pub const UP: u32 = SPECIAL + 9;
    pub const DOWN: u32 = SPECIAL + 10;
    pub const HOME: u32 = SPECIAL + 11;
    pub const END: u32 = SPECIAL + 12;
    pub const PAGE_UP: u32 = SPECIAL + 13;
    pub const PAGE_DOWN: u32 = SPECIAL + 14;
    /// `F + n` is the function key Fn.
    pub const F: u32 = 0x12_0000;

    pub const SHIFT: u32 = 1;
    pub const CTRL: u32 = 2;
    pub const ALT: u32 = 4;
}

/// An event as Lean's `RawEvent` holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raw {
    pub kind: u32,
    pub code: u32,
    pub modifiers: u32,
    pub width: u32,
    pub height: u32,
    pub text: String,
}

impl Raw {
    fn new(kind: u32) -> Raw {
        Raw {
            kind,
            code: 0,
            modifiers: 0,
            width: 0,
            height: 0,
            text: String::new(),
        }
    }
}

fn key_code(k: KeyCode) -> Option<u32> {
    use code::*;
    Some(match k {
        KeyCode::Char(c) => c as u32,
        KeyCode::Enter => ENTER,
        KeyCode::Esc => ESC,
        KeyCode::Backspace => BACKSPACE,
        KeyCode::Tab => TAB,
        KeyCode::BackTab => BACK_TAB,
        KeyCode::Delete => DELETE,
        KeyCode::Insert => INSERT,
        KeyCode::Left => LEFT,
        KeyCode::Right => RIGHT,
        KeyCode::Up => UP,
        KeyCode::Down => DOWN,
        KeyCode::Home => HOME,
        KeyCode::End => END,
        KeyCode::PageUp => PAGE_UP,
        KeyCode::PageDown => PAGE_DOWN,
        KeyCode::F(n) => F + n as u32,
        _ => return None,
    })
}

fn modifiers(m: KeyModifiers) -> u32 {
    let mut r = 0;
    if m.contains(KeyModifiers::SHIFT) {
        r |= code::SHIFT;
    }
    if m.contains(KeyModifiers::CONTROL) {
        r |= code::CTRL;
    }
    if m.contains(KeyModifiers::ALT) {
        r |= code::ALT;
    }
    r
}

fn button(b: MouseButton) -> u32 {
    use code::*;
    match b {
        MouseButton::Left => BUTTON_LEFT,
        MouseButton::Right => BUTTON_RIGHT,
        MouseButton::Middle => BUTTON_MIDDLE,
    }
}

fn mouse_code(k: MouseEventKind) -> u32 {
    use code::*;
    match k {
        MouseEventKind::Down(b) => MOUSE_DOWN | button(b) << BUTTON_SHIFT,
        MouseEventKind::Up(b) => MOUSE_UP | button(b) << BUTTON_SHIFT,
        MouseEventKind::Drag(b) => MOUSE_DRAG | button(b) << BUTTON_SHIFT,
        MouseEventKind::Moved => MOUSE_MOVED,
        MouseEventKind::ScrollDown => SCROLL_DOWN,
        MouseEventKind::ScrollUp => SCROLL_UP,
        MouseEventKind::ScrollLeft => SCROLL_LEFT,
        MouseEventKind::ScrollRight => SCROLL_RIGHT,
    }
}

/// The events Lean sees: key presses, resizes, pastes, the mouse and focus. Others give `None`.
pub fn encode(e: &Event) -> Option<Raw> {
    match e {
        Event::Key(KeyEvent {
            code: k,
            modifiers: m,
            kind,
            ..
        }) if *kind != KeyEventKind::Release => Some(Raw {
            code: key_code(*k)?,
            modifiers: modifiers(*m),
            ..Raw::new(code::KIND_KEY)
        }),
        Event::Resize(w, h) => Some(Raw {
            width: *w as u32,
            height: *h as u32,
            ..Raw::new(code::KIND_RESIZE)
        }),
        Event::Paste(s) => Some(Raw {
            text: s.clone(),
            ..Raw::new(code::KIND_PASTE)
        }),
        Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: m,
        }) => Some(Raw {
            code: mouse_code(*kind),
            modifiers: modifiers(*m),
            width: *column as u32,
            height: *row as u32,
            ..Raw::new(code::KIND_MOUSE)
        }),
        Event::FocusGained => Some(Raw {
            code: 1,
            ..Raw::new(code::KIND_FOCUS)
        }),
        Event::FocusLost => Some(Raw::new(code::KIND_FOCUS)),
        Event::Key(_) => None,
    }
}

/// One event of every kind and key, in the order `CrosstermFfi.Tests` expects.
pub fn samples() -> Vec<Event> {
    let key = |k, m| Event::Key(KeyEvent::new(k, m));
    let none = KeyModifiers::NONE;
    let mut v = vec![
        key(KeyCode::Char('a'), none),
        key(KeyCode::Char('Z'), KeyModifiers::SHIFT),
        key(KeyCode::Char('c'), KeyModifiers::CONTROL),
        key(
            KeyCode::Char('x'),
            KeyModifiers::ALT | KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ),
        key(KeyCode::Char('é'), none),
        key(KeyCode::Char('🦀'), none),
    ];
    for k in [
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Backspace,
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Delete,
        KeyCode::Insert,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::F(1),
        KeyCode::F(12),
    ] {
        v.push(key(k, none));
    }
    v.push(Event::Resize(80, 24));
    v.push(Event::Paste("hello\nworld".into()));
    let mouse = |kind, column, row, modifiers| {
        Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers,
        })
    };
    v.push(mouse(MouseEventKind::Down(MouseButton::Left), 3, 4, none));
    v.push(mouse(
        MouseEventKind::Up(MouseButton::Right),
        0,
        0,
        KeyModifiers::CONTROL,
    ));
    v.push(mouse(
        MouseEventKind::Drag(MouseButton::Middle),
        79,
        23,
        none,
    ));
    v.push(mouse(MouseEventKind::Moved, 1, 2, none));
    v.push(mouse(MouseEventKind::ScrollDown, 5, 6, none));
    v.push(mouse(MouseEventKind::ScrollUp, 5, 6, KeyModifiers::SHIFT));
    v.push(mouse(MouseEventKind::ScrollLeft, 0, 1, none));
    v.push(mouse(MouseEventKind::ScrollRight, 1, 0, none));
    v.push(Event::FocusGained);
    v.push(Event::FocusLost);
    v
}

unsafe fn string(s: &str) -> *mut lean_object {
    lean_mk_string_from_bytes(s.as_ptr(), s.len())
}

unsafe fn raw_event(r: &Raw) -> *mut lean_object {
    let o = lean_alloc_ctor(0, 1, 20);
    lean_ctor_set(o, 0, string(&r.text));
    let base = size_of::<*mut lean_object>() as u32;
    for (i, v) in [r.kind, r.code, r.modifiers, r.width, r.height]
        .into_iter()
        .enumerate()
    {
        lean_ctor_set_uint32(o, base + 4 * i as u32, v);
    }
    o
}

unsafe fn some(v: *mut lean_object) -> *mut lean_object {
    let o = lean_alloc_ctor(1, 1, 0);
    lean_ctor_set(o, 0, v);
    o
}

unsafe fn io(r: io::Result<*mut lean_object>) -> *mut lean_object {
    match r {
        Ok(v) => lean_io_result_mk_ok(v),
        Err(e) => lean_io_result_mk_error(lean_mk_io_user_error(string(&e.to_string()))),
    }
}

unsafe fn io_unit(r: io::Result<()>) -> *mut lean_object {
    io(r.map(|()| lean_box(0)))
}

#[no_mangle]
pub unsafe extern "C" fn cl_enable_raw_mode() -> *mut lean_object {
    io_unit(terminal::enable_raw_mode())
}

#[no_mangle]
pub unsafe extern "C" fn cl_disable_raw_mode() -> *mut lean_object {
    io_unit(terminal::disable_raw_mode())
}

#[no_mangle]
pub unsafe extern "C" fn cl_enter_alternate_screen() -> *mut lean_object {
    io_unit(execute!(stdout(), terminal::EnterAlternateScreen))
}

#[no_mangle]
pub unsafe extern "C" fn cl_leave_alternate_screen() -> *mut lean_object {
    io_unit(execute!(stdout(), terminal::LeaveAlternateScreen))
}

#[no_mangle]
pub unsafe extern "C" fn cl_show_cursor() -> *mut lean_object {
    io_unit(execute!(stdout(), cursor::Show))
}

#[no_mangle]
pub unsafe extern "C" fn cl_hide_cursor() -> *mut lean_object {
    io_unit(execute!(stdout(), cursor::Hide))
}

#[no_mangle]
pub unsafe extern "C" fn cl_enable_bracketed_paste() -> *mut lean_object {
    io_unit(execute!(stdout(), event::EnableBracketedPaste))
}

#[no_mangle]
pub unsafe extern "C" fn cl_disable_bracketed_paste() -> *mut lean_object {
    io_unit(execute!(stdout(), event::DisableBracketedPaste))
}

#[no_mangle]
pub unsafe extern "C" fn cl_enable_mouse_capture() -> *mut lean_object {
    io_unit(execute!(stdout(), event::EnableMouseCapture))
}

#[no_mangle]
pub unsafe extern "C" fn cl_disable_mouse_capture() -> *mut lean_object {
    io_unit(execute!(stdout(), event::DisableMouseCapture))
}

#[no_mangle]
pub unsafe extern "C" fn cl_enable_focus_change() -> *mut lean_object {
    io_unit(execute!(stdout(), event::EnableFocusChange))
}

#[no_mangle]
pub unsafe extern "C" fn cl_disable_focus_change() -> *mut lean_object {
    io_unit(execute!(stdout(), event::DisableFocusChange))
}

#[no_mangle]
pub unsafe extern "C" fn cl_size() -> *mut lean_object {
    io(terminal::size().map(|(w, h)| {
        let p = lean_alloc_ctor(0, 2, 0);
        lean_ctor_set(p, 0, lean_box(w as usize));
        lean_ctor_set(p, 1, lean_box(h as usize));
        p
    }))
}

/// Wait up to `timeout_ms` for an event Lean sees, skipping the others.
fn poll(timeout_ms: u32) -> io::Result<Option<Raw>> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if !event::poll(left)? {
            return Ok(None);
        }
        if let Some(r) = encode(&event::read()?) {
            return Ok(Some(r));
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn cl_poll(timeout_ms: u32) -> *mut lean_object {
    io(poll(timeout_ms).map(|r| match r {
        Some(r) => some(raw_event(&r)),
        None => lean_box(0),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn cl_sample_events() -> *mut lean_object {
    let mut a = lean_mk_empty_array();
    for e in samples() {
        a = lean_array_push(
            a,
            raw_event(&encode(&e).expect("samples are all encodable")),
        );
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_encode() {
        let raws: Vec<Raw> = samples().iter().map(|e| encode(e).unwrap()).collect();
        assert_eq!(raws[0].code, 'a' as u32);
        assert_eq!(raws[3].modifiers, code::SHIFT | code::CTRL | code::ALT);
        assert_eq!(raws[5].code, '🦀' as u32);
        let specials: Vec<u32> = raws[6..21].iter().map(|r| r.code).collect();
        assert_eq!(
            specials,
            (code::ENTER..=code::PAGE_DOWN).collect::<Vec<_>>()
        );
        assert_eq!(raws[21].code, code::F + 1);
        assert_eq!(raws[22].code, code::F + 12);
        assert_eq!(
            (raws[23].kind, raws[23].width, raws[23].height),
            (code::KIND_RESIZE, 80, 24)
        );
        assert_eq!(
            (raws[24].kind, raws[24].text.as_str()),
            (code::KIND_PASTE, "hello\nworld")
        );
        assert_eq!(
            (
                raws[25].kind,
                raws[25].code,
                raws[25].width,
                raws[25].height
            ),
            (code::KIND_MOUSE, code::MOUSE_DOWN, 3, 4)
        );
        assert_eq!(
            (raws[26].code, raws[26].modifiers),
            (
                code::MOUSE_UP | code::BUTTON_RIGHT << code::BUTTON_SHIFT,
                code::CTRL
            )
        );
        assert_eq!(
            raws[27].code,
            code::MOUSE_DRAG | code::BUTTON_MIDDLE << code::BUTTON_SHIFT
        );
        let kinds: Vec<u32> = raws[28..33].iter().map(|r| r.code).collect();
        assert_eq!(
            kinds,
            (code::MOUSE_MOVED..=code::SCROLL_RIGHT).collect::<Vec<_>>()
        );
        assert_eq!((raws[33].kind, raws[33].code), (code::KIND_FOCUS, 1));
        assert_eq!((raws[34].kind, raws[34].code), (code::KIND_FOCUS, 0));
    }

    #[test]
    fn characters_stay_below_special() {
        assert!((char::MAX as u32) < code::SPECIAL);
    }

    #[test]
    fn others_are_skipped() {
        let release = KeyEvent::new_with_kind(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );
        assert_eq!(encode(&Event::Key(release)), None);
        assert_eq!(
            encode(&Event::Key(KeyEvent::new(
                KeyCode::CapsLock,
                KeyModifiers::NONE
            ))),
            None
        );
    }
}
