//! Winsock entry points. The network is reported as unavailable.

use runtime::Context;

use crate::{Ptr, stub};

const SOCKET_ERROR: i32 = -1;
const INVALID_SOCKET: u32 = 0xFFFF_FFFF;
const WSASYSNOTREADY: i32 = 10091;

#[win32_derive::dllexport]
pub fn WSAStartup(_ctx: &mut Context, _wVersionRequested: u32, _lpWSAData: u32) -> i32 {
    stub!(WSASYSNOTREADY)
}

#[win32_derive::dllexport]
pub fn WSACleanup(_ctx: &mut Context) -> i32 {
    0
}

#[win32_derive::dllexport]
pub fn socket(_ctx: &mut Context, _af: i32, _type_: i32, _protocol: i32) -> u32 {
    stub!(INVALID_SOCKET)
}

#[win32_derive::dllexport]
pub fn connect(_ctx: &mut Context, _s: u32, _name: u32, _namelen: i32) -> i32 {
    stub!(SOCKET_ERROR)
}

#[win32_derive::dllexport]
pub fn send(_ctx: &mut Context, _s: u32, _buf: u32, _len: i32, _flags: i32) -> i32 {
    stub!(SOCKET_ERROR)
}

#[win32_derive::dllexport]
pub fn recv(_ctx: &mut Context, _s: u32, _buf: u32, _len: i32, _flags: i32) -> i32 {
    stub!(SOCKET_ERROR)
}

#[win32_derive::dllexport]
pub fn shutdown(_ctx: &mut Context, _s: u32, _how: i32) -> i32 {
    stub!(SOCKET_ERROR)
}

#[win32_derive::dllexport]
pub fn closesocket(_ctx: &mut Context, _s: u32) -> i32 {
    0
}

#[win32_derive::dllexport]
pub fn gethostbyname(_ctx: &mut Context, _name: Ptr<u8>) -> u32 {
    stub!(0)
}

#[win32_derive::dllexport]
pub fn htons(_ctx: &mut Context, hostshort: u32) -> u32 {
    (hostshort as u16).swap_bytes() as u32
}

#[win32_derive::dllexport]
pub fn htonl(_ctx: &mut Context, hostlong: u32) -> u32 {
    hostlong.swap_bytes()
}
