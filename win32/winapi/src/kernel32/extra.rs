//! kernel32 functions needed by the Lemonade port.

use std::io::{Seek, SeekFrom};

use runtime::Context;

use crate::{
    Ptr,
    kernel32::{self, HMODULE, HRSRC, Object, lock},
    stub,
};

const INVALID_HANDLE_VALUE: u32 = 0xFFFF_FFFF;
const INVALID_FILE_SIZE: u32 = 0xFFFF_FFFF;
/// Seconds between the FILETIME epoch (1601) and the Unix epoch (1970).
const FILETIME_UNIX_OFFSET: i64 = 11_644_473_600;

#[win32_derive::dllexport]
pub fn CreateNamedPipeA(
    _ctx: &mut Context,
    _lpName: Ptr<u8>,
    _dwOpenMode: u32,
    _dwPipeMode: u32,
    _nMaxInstances: u32,
    _nOutBufferSize: u32,
    _nInBufferSize: u32,
    _nDefaultTimeOut: u32,
    _lpSecurityAttributes: u32,
) -> u32 {
    stub!(INVALID_HANDLE_VALUE)
}

#[win32_derive::dllexport]
pub fn ConnectNamedPipe(_ctx: &mut Context, _hNamedPipe: u32, _lpOverlapped: u32) -> bool {
    stub!(false)
}

#[win32_derive::dllexport]
pub fn CreateProcessA(
    _ctx: &mut Context,
    _lpApplicationName: Ptr<u8>,
    _lpCommandLine: Ptr<u8>,
    _lpProcessAttributes: u32,
    _lpThreadAttributes: u32,
    _bInheritHandles: bool,
    _dwCreationFlags: u32,
    _lpEnvironment: u32,
    _lpCurrentDirectory: Ptr<u8>,
    _lpStartupInfo: u32,
    _lpProcessInformation: u32,
) -> bool {
    stub!(false)
}

fn resource_name(ctx: &Context, ptr: &Ptr<u8>) -> ResourceKey {
    if ptr.addr >> 16 == 0 {
        ResourceKey::Id(ptr.addr)
    } else {
        let name = ctx.memory.read_str(ptr.addr);
        // "#123" is the string spelling of an integer resource id.
        match name.strip_prefix('#').and_then(|n| n.parse().ok()) {
            Some(id) => ResourceKey::Id(id),
            None => ResourceKey::Name(name.encode_utf16().collect()),
        }
    }
}

enum ResourceKey {
    Id(u32),
    Name(Vec<u16>),
}

impl ResourceKey {
    fn wide(&self) -> Option<widestring::U16String> {
        match self {
            ResourceKey::Id(_) => None,
            ResourceKey::Name(name) => {
                let upper: Vec<u16> = String::from_utf16_lossy(name)
                    .to_uppercase()
                    .encode_utf16()
                    .collect();
                Some(widestring::U16String::from_vec(upper))
            }
        }
    }

    fn id(&self) -> u32 {
        match self {
            ResourceKey::Id(id) => *id,
            ResourceKey::Name(_) => 0,
        }
    }
}

#[win32_derive::dllexport]
pub fn FindResourceA(
    ctx: &mut Context,
    _hModule: HMODULE,
    lpName: Ptr<u8>,
    lpType: Ptr<u8>,
) -> HRSRC {
    let name = resource_name(ctx, &lpName);
    let typ = resource_name(ctx, &lpType);
    let (name_wide, typ_wide) = (name.wide(), typ.wide());
    let name = match &name_wide {
        Some(wide) => exe::ResourceName::Name(wide),
        None => exe::ResourceName::Id(name.id()),
    };
    let typ = match &typ_wide {
        Some(wide) => exe::ResourceName::Name(wide),
        None => exe::ResourceName::Id(typ.id()),
    };
    match kernel32::lock().find_resource(ctx, typ, name) {
        Some(buf) => unsafe { buf.as_ptr().byte_offset_from(ctx.memory.as_ptr()) as u32 },
        None => {
            log::warn!("FindResourceA: not found");
            0
        }
    }
}

#[win32_derive::dllexport]
pub fn GetFileSize(ctx: &mut Context, hFile: crate::HANDLE, lpFileSizeHigh: Ptr<u32>) -> u32 {
    let mut kernel32 = lock();
    let Some(Object::File(file)) = kernel32.objects.get_mut(hFile) else {
        log::warn!("GetFileSize({hFile:?}): unknown handle");
        return INVALID_FILE_SIZE;
    };
    let size = (|| {
        let pos = file.stream_position()?;
        let end = file.seek(SeekFrom::End(0))?;
        file.seek(SeekFrom::Start(pos))?;
        Ok::<u64, std::io::Error>(end)
    })();
    drop(kernel32);
    match size {
        Ok(size) => {
            if lpFileSizeHigh.addr != 0 {
                lpFileSizeHigh.write(&mut ctx.memory, (size >> 32) as u32);
            }
            size as u32
        }
        Err(err) => {
            log::warn!("GetFileSize({hFile:?}): {err}");
            INVALID_FILE_SIZE
        }
    }
}

/// Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (month as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn write_system_time(ctx: &mut Context, addr: u32) {
    let now = host::unix_time_ms();
    let secs = (now / 1000) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let weekday = (days + 4).rem_euclid(7);
    let fields = [
        year as u16,
        month as u16,
        weekday as u16,
        day as u16,
        (rem / 3600) as u16,
        (rem % 3600 / 60) as u16,
        (rem % 60) as u16,
        (now % 1000) as u16,
    ];
    for (i, field) in fields.iter().enumerate() {
        ctx.memory.write::<u16>(addr + i as u32 * 2, *field);
    }
}

#[win32_derive::dllexport]
pub fn GetSystemTime(ctx: &mut Context, lpSystemTime: Ptr<u8>) {
    write_system_time(ctx, lpSystemTime.addr);
}

#[win32_derive::dllexport]
pub fn GetLocalTime(ctx: &mut Context, lpSystemTime: Ptr<u8>) {
    // GetTimeZoneInformation reports UTC, so local time matches it.
    write_system_time(ctx, lpSystemTime.addr);
}

#[win32_derive::dllexport]
pub fn SystemTimeToFileTime(
    ctx: &mut Context,
    lpSystemTime: Ptr<u8>,
    lpFileTime: Ptr<u64>,
) -> bool {
    let a = lpSystemTime.addr;
    let field = |i: u32| ctx.memory.read::<u16>(a + i * 2) as i64;
    let days = days_from_civil(field(0), field(1) as u32, field(3) as u32);
    let secs = days * 86_400 + field(4) * 3600 + field(5) * 60 + field(6);
    let ticks = (secs + FILETIME_UNIX_OFFSET) * 10_000_000 + field(7) * 10_000;
    lpFileTime.write(&mut ctx.memory, ticks as u64).is_some()
}

#[win32_derive::dllexport]
pub fn GetModuleFileNameW(
    _ctx: &mut Context,
    _hModule: HMODULE,
    _lpFilename: Ptr<u16>,
    _nSize: u32,
) -> u32 {
    stub!(0)
}

#[win32_derive::dllexport]
pub fn GetProfileIntA(
    _ctx: &mut Context,
    _lpAppName: Ptr<u8>,
    _lpKeyName: Ptr<u8>,
    nDefault: i32,
) -> i32 {
    nDefault
}

#[win32_derive::dllexport]
pub fn WriteProfileStringA(
    _ctx: &mut Context,
    _lpAppName: Ptr<u8>,
    _lpKeyName: Ptr<u8>,
    _lpString: Ptr<u8>,
) -> bool {
    stub!(true)
}

#[win32_derive::dllexport]
pub fn GetSystemInfo(ctx: &mut Context, lpSystemInfo: Ptr<u8>) {
    // SYSTEM_INFO for a single-processor x86 machine.
    let fields: [u32; 9] = [
        0,          // wProcessorArchitecture = INTEL
        0x1000,     // dwPageSize
        0x0001_0000, // lpMinimumApplicationAddress
        0x7FFE_FFFF, // lpMaximumApplicationAddress
        1,          // dwActiveProcessorMask
        1,          // dwNumberOfProcessors
        586,        // dwProcessorType = PROCESSOR_INTEL_PENTIUM
        0x0001_0000, // dwAllocationGranularity
        0x0006_0006, // wProcessorLevel, wProcessorRevision
    ];
    for (i, field) in fields.iter().enumerate() {
        ctx.memory.write::<u32>(lpSystemInfo.addr + i as u32 * 4, *field);
    }
}

#[win32_derive::dllexport]
pub fn GlobalMemoryStatus(ctx: &mut Context, lpBuffer: Ptr<u8>) {
    const MB: u32 = 1 << 20;
    let fields: [u32; 8] = [
        32,
        10,
        256 * MB,
        192 * MB,
        512 * MB,
        448 * MB,
        2047 * MB,
        1984 * MB,
    ];
    for (i, field) in fields.iter().enumerate() {
        ctx.memory.write::<u32>(lpBuffer.addr + i as u32 * 4, *field);
    }
}

#[win32_derive::dllexport]
pub fn GetUserDefaultLangID(_ctx: &mut Context) -> u32 {
    0x0409 // en-US
}

#[win32_derive::dllexport]
pub fn IsBadCodePtr(_ctx: &mut Context, _lpfn: u32) -> bool {
    false
}

#[win32_derive::dllexport]
pub fn IsBadReadPtr(_ctx: &mut Context, lp: u32, _ucb: u32) -> bool {
    lp < 0x1000
}

#[win32_derive::dllexport]
pub fn IsBadWritePtr(_ctx: &mut Context, lp: u32, _ucb: u32) -> bool {
    lp < 0x1000
}

#[win32_derive::dllexport]
pub fn SetUnhandledExceptionFilter(_ctx: &mut Context, _lpTopLevelExceptionFilter: u32) -> u32 {
    stub!(0)
}

#[win32_derive::dllexport]
pub fn lstrcmpiA(ctx: &mut Context, lpString1: Ptr<u8>, lpString2: Ptr<u8>) -> i32 {
    let a = ctx.memory.read_cstr(lpString1.addr).to_ascii_lowercase();
    let b = ctx.memory.read_cstr(lpString2.addr).to_ascii_lowercase();
    match a.cmp(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}
