//! Windows CE remote API entry points. No device is ever attached.

use runtime::Context;

use crate::stub;

const E_FAIL: u32 = 0x8000_4005;
const INVALID_HANDLE_VALUE: u32 = 0xFFFF_FFFF;
const ERROR_FILE_NOT_FOUND: u32 = 2;

#[win32_derive::dllexport]
pub fn CeRapiInitEx(_ctx: &mut Context, _pRapiInit: u32) -> u32 {
    stub!(E_FAIL)
}

#[win32_derive::dllexport]
pub fn CeRapiUninit(_ctx: &mut Context) -> u32 {
    0
}

#[win32_derive::dllexport]
pub fn CeCreateFile(
    _ctx: &mut Context,
    _lpFileName: u32,
    _dwDesiredAccess: u32,
    _dwShareMode: u32,
    _lpSecurityAttributes: u32,
    _dwCreationDisposition: u32,
    _dwFlagsAndAttributes: u32,
    _hTemplateFile: u32,
) -> u32 {
    stub!(INVALID_HANDLE_VALUE)
}

#[win32_derive::dllexport]
pub fn CeCloseHandle(_ctx: &mut Context, _hObject: u32) -> bool {
    true
}

#[win32_derive::dllexport]
pub fn CeGetFileSize(_ctx: &mut Context, _hFile: u32, _lpFileSizeHigh: u32) -> u32 {
    stub!(INVALID_HANDLE_VALUE)
}

#[win32_derive::dllexport]
pub fn CeReadFile(
    _ctx: &mut Context,
    _hFile: u32,
    _lpBuffer: u32,
    _nNumberOfBytesToRead: u32,
    _lpNumberOfBytesRead: u32,
    _lpOverlapped: u32,
) -> bool {
    stub!(false)
}

#[win32_derive::dllexport]
pub fn CeWriteFile(
    _ctx: &mut Context,
    _hFile: u32,
    _lpBuffer: u32,
    _nNumberOfBytesToWrite: u32,
    _lpNumberOfBytesWritten: u32,
    _lpOverlapped: u32,
) -> bool {
    stub!(false)
}

#[win32_derive::dllexport]
pub fn CeRegOpenKeyEx(
    _ctx: &mut Context,
    _hKey: u32,
    _lpszSubKey: u32,
    _ulOptions: u32,
    _samDesired: u32,
    _phkResult: u32,
) -> u32 {
    stub!(ERROR_FILE_NOT_FOUND)
}

#[win32_derive::dllexport]
pub fn CeRegQueryValueEx(
    _ctx: &mut Context,
    _hKey: u32,
    _lpValueName: u32,
    _lpReserved: u32,
    _lpType: u32,
    _lpData: u32,
    _lpcbData: u32,
) -> u32 {
    stub!(ERROR_FILE_NOT_FOUND)
}

#[win32_derive::dllexport]
pub fn CeRegCloseKey(_ctx: &mut Context, _hKey: u32) -> u32 {
    0
}
