//! user32 functions needed by the Lemonade port.

use runtime::Context;

use super::*;
use crate::{Ptr, RECT, stub};

#[win32_derive::dllexport]
pub fn AdjustWindowRectEx(
    _ctx: &mut Context,
    _lpRect: Ptr<RECT>,
    _dwStyle: u32,
    _bMenu: bool,
    _dwExStyle: u32,
) -> bool {
    // Windows have no frame here, so the client rect is the window rect.
    true
}

#[win32_derive::dllexport]
pub fn ChangeDisplaySettingsA(_ctx: &mut Context, _lpDevMode: u32, _dwFlags: u32) -> i32 {
    0 // DISP_CHANGE_SUCCESSFUL
}

#[win32_derive::dllexport]
pub fn OpenClipboard(_ctx: &mut Context, _hWndNewOwner: HWND) -> bool {
    stub!(false)
}

#[win32_derive::dllexport]
pub fn CloseClipboard(_ctx: &mut Context) -> bool {
    stub!(true)
}

#[win32_derive::dllexport]
pub fn EmptyClipboard(_ctx: &mut Context) -> bool {
    stub!(true)
}

#[win32_derive::dllexport]
pub fn SetClipboardData(_ctx: &mut Context, _uFormat: u32, _hMem: u32) -> u32 {
    stub!(0)
}

#[win32_derive::dllexport]
pub fn RegisterClipboardFormatA(_ctx: &mut Context, _lpszFormat: Ptr<u8>) -> u32 {
    stub!(0xC000)
}

#[win32_derive::dllexport]
pub fn CloseWindow(_ctx: &mut Context, _hWnd: HWND) -> bool {
    stub!(true)
}

#[win32_derive::dllexport]
pub fn GetDlgItemTextA(
    ctx: &mut Context,
    _hDlg: HWND,
    _nIDDlgItem: i32,
    lpString: Ptr<u8>,
    cchMax: i32,
) -> u32 {
    if cchMax > 0 {
        ctx.memory.write::<u8>(lpString.addr, 0);
    }
    stub!(0)
}

#[win32_derive::dllexport]
pub fn SetDlgItemTextA(
    _ctx: &mut Context,
    _hDlg: HWND,
    _nIDDlgItem: i32,
    _lpString: Ptr<u8>,
) -> bool {
    stub!(true)
}

#[win32_derive::dllexport]
pub fn GetKeyboardType(_ctx: &mut Context, nTypeFlag: i32) -> i32 {
    match nTypeFlag {
        0 => 4,  // enhanced 101/102-key keyboard
        1 => 0,  // no subtype
        2 => 12, // function keys
        _ => 0,
    }
}

#[win32_derive::dllexport]
pub fn GetWindowLongA(_ctx: &mut Context, _hWnd: HWND, _nIndex: i32) -> i32 {
    stub!(0)
}

#[win32_derive::dllexport]
pub fn GetWindowRect(ctx: &mut Context, _hWnd: HWND, lpRect: Ptr<RECT>) -> bool {
    let rect = {
        let window = state().window.borrow();
        match window.as_ref() {
            Some(window) => window.borrow().rect(),
            None => return false,
        }
    };
    lpRect.write(&mut ctx.memory, rect).is_some()
}

#[win32_derive::dllexport]
pub fn GetWindowTextA(
    ctx: &mut Context,
    _hWnd: HWND,
    lpString: Ptr<u8>,
    nMaxCount: i32,
) -> i32 {
    if nMaxCount > 0 {
        ctx.memory.write::<u8>(lpString.addr, 0);
    }
    0
}

#[win32_derive::dllexport]
pub fn IsWindowEnabled(_ctx: &mut Context, _hWnd: HWND) -> bool {
    true
}

#[win32_derive::dllexport]
pub fn MessageBeep(_ctx: &mut Context, _uType: u32) -> bool {
    true
}

#[win32_derive::dllexport]
pub fn SendMessageA(
    _ctx: &mut Context,
    _hWnd: HWND,
    _Msg: u32,
    _wParam: u32,
    _lParam: u32,
) -> u32 {
    stub!(0)
}

#[win32_derive::dllexport]
pub fn SetForegroundWindow(_ctx: &mut Context, _hWnd: HWND) -> bool {
    true
}

#[win32_derive::dllexport]
pub fn SystemParametersInfoA(
    _ctx: &mut Context,
    _uiAction: u32,
    _uiParam: u32,
    _pvParam: u32,
    _fWinIni: u32,
) -> bool {
    stub!(false)
}
