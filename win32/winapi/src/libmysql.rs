//! MySQL client entry points. There is no server, so connecting always fails.

use runtime::Context;

use crate::{Ptr, stub};

#[win32_derive::dllexport]
pub fn mysql_init(_ctx: &mut Context, mysql: u32) -> u32 {
    stub!(mysql)
}

#[win32_derive::dllexport]
pub fn mysql_close(_ctx: &mut Context, _mysql: u32) {}

#[win32_derive::dllexport]
pub fn mysql_real_connect(
    _ctx: &mut Context,
    _mysql: u32,
    _host: Ptr<u8>,
    _user: Ptr<u8>,
    _passwd: Ptr<u8>,
    _db: Ptr<u8>,
    _port: u32,
    _unix_socket: Ptr<u8>,
    _clientflag: u32,
) -> u32 {
    stub!(0)
}

#[win32_derive::dllexport]
pub fn mysql_query(_ctx: &mut Context, _mysql: u32, _q: Ptr<u8>) -> i32 {
    stub!(1)
}

#[win32_derive::dllexport]
pub fn mysql_store_result(_ctx: &mut Context, _mysql: u32) -> u32 {
    stub!(0)
}

#[win32_derive::dllexport]
pub fn mysql_free_result(_ctx: &mut Context, _result: u32) {}

#[win32_derive::dllexport]
pub fn mysql_field_count(_ctx: &mut Context, _mysql: u32) -> u32 {
    0
}

#[win32_derive::dllexport]
pub fn mysql_errno(_ctx: &mut Context, _mysql: u32) -> u32 {
    2003 // CR_CONN_HOST_ERROR
}

#[win32_derive::dllexport]
pub fn mysql_error(_ctx: &mut Context, _mysql: u32) -> u32 {
    // The caller formats this as a string; the image header begins with "MZ".
    0x0040_0000
}

#[win32_derive::dllexport]
pub fn mysql_fetch_row(_ctx: &mut Context, _result: u32) -> u32 {
    0
}

#[win32_derive::dllexport]
pub fn mysql_num_rows(_ctx: &mut Context, _result: u32) -> u32 {
    0
}

#[win32_derive::dllexport]
pub fn mysql_num_fields(_ctx: &mut Context, _result: u32) -> u32 {
    0
}

#[win32_derive::dllexport]
pub fn mysql_field_seek(_ctx: &mut Context, _result: u32, _offset: u32) -> u32 {
    0
}

#[win32_derive::dllexport]
pub fn mysql_fetch_field(_ctx: &mut Context, _result: u32) -> u32 {
    0
}
