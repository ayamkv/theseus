//! The ANSI registry API, backed by a small file next to the program.

use std::{
    collections::{BTreeMap, HashMap},
    io::Write,
    sync::Mutex,
};

use runtime::Context;

use crate::{Ptr, kernel32::resolve_path};

const ERROR_SUCCESS: u32 = 0;
const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_MORE_DATA: u32 = 234;
/// Opened key handles start here, clear of the predefined HKEY_* values'
/// meaning and of zero.
const FIRST_HANDLE: u32 = 0x100;
const REGISTRY_FILE: &str = "C:\\registry.dat";

struct Value {
    kind: u32,
    data: Vec<u8>,
}

#[derive(Default)]
struct State {
    /// Key path to its values. A key with no values still has an entry.
    keys: BTreeMap<String, BTreeMap<String, Value>>,
    open: HashMap<u32, String>,
    next_handle: u32,
    loaded: bool,
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

impl State {
    fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        self.next_handle = FIRST_HANDLE;
        let Ok(bytes) = host::fs::read(resolve_path(REGISTRY_FILE)) else {
            return;
        };
        // One record per line: hex(key) [hex(name) kind hex(data)]
        for line in String::from_utf8_lossy(&bytes).lines() {
            let mut fields = line.split(' ');
            let Some(key) = fields.next().and_then(unhex) else {
                continue;
            };
            let key = String::from_utf8_lossy(&key).into_owned();
            let values = self.keys.entry(key).or_default();
            if let (Some(name), Some(kind), Some(data)) =
                (fields.next(), fields.next(), fields.next())
            {
                if let (Some(name), Ok(kind), Some(data)) = (unhex(name), kind.parse(), unhex(data))
                {
                    let name = String::from_utf8_lossy(&name).into_owned();
                    values.insert(name, Value { kind, data });
                }
            }
        }
    }

    fn save(&self) {
        let mut text = String::new();
        for (key, values) in &self.keys {
            let key = hex(key.as_bytes());
            text.push_str(&format!("{key}\n"));
            for (name, value) in values {
                let name = hex(name.as_bytes());
                let data = hex(&value.data);
                text.push_str(&format!("{key} {name} {} {data}\n", value.kind));
            }
        }
        let file = host::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(resolve_path(REGISTRY_FILE));
        match file {
            Ok(mut file) => {
                if let Err(err) = file.write_all(text.as_bytes()).and_then(|_| file.flush()) {
                    log::warn!("registry: {err}");
                }
            }
            Err(err) => log::warn!("registry: {err}"),
        }
    }

    fn path(&self, hkey: u32, sub_key: &str) -> String {
        let base = match self.open.get(&hkey) {
            Some(path) => path.clone(),
            None => format!("{hkey:08x}"),
        };
        let sub_key = sub_key.trim_matches('\\').to_lowercase();
        if sub_key.is_empty() {
            base
        } else {
            format!("{base}\\{sub_key}")
        }
    }

    fn open_handle(&mut self, path: String) -> u32 {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.open.insert(handle, path);
        handle
    }
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut state = STATE.lock().unwrap();
    let state = state.get_or_insert_with(State::default);
    state.load();
    f(state)
}

fn read_name(ctx: &Context, ptr: &Ptr<u8>) -> String {
    if ptr.addr == 0 {
        String::new()
    } else {
        ctx.memory.read_str(ptr.addr)
    }
}

fn create_key(ctx: &mut Context, hKey: u32, lpSubKey: &Ptr<u8>, phkResult: &Ptr<u32>) -> bool {
    let sub_key = read_name(ctx, lpSubKey);
    let (handle, created) = with(|state| {
        let path = state.path(hKey, &sub_key);
        let created = !state.keys.contains_key(&path);
        if created {
            state.keys.insert(path.clone(), BTreeMap::new());
            state.save();
        }
        (state.open_handle(path), created)
    });
    phkResult.write(&mut ctx.memory, handle);
    created
}

#[win32_derive::dllexport]
pub fn RegCreateKeyA(
    ctx: &mut Context,
    hKey: u32,
    lpSubKey: Ptr<u8>,
    phkResult: Ptr<u32>,
) -> u32 {
    create_key(ctx, hKey, &lpSubKey, &phkResult);
    ERROR_SUCCESS
}

#[win32_derive::dllexport]
pub fn RegCreateKeyExA(
    ctx: &mut Context,
    hKey: u32,
    lpSubKey: Ptr<u8>,
    _Reserved: u32,
    _lpClass: u32,
    _dwOptions: u32,
    _samDesired: u32,
    _lpSecurityAttributes: u32,
    phkResult: Ptr<u32>,
    lpdwDisposition: Ptr<u32>,
) -> u32 {
    let created = create_key(ctx, hKey, &lpSubKey, &phkResult);
    if lpdwDisposition.addr != 0 {
        // REG_CREATED_NEW_KEY or REG_OPENED_EXISTING_KEY
        lpdwDisposition.write(&mut ctx.memory, if created { 1 } else { 2 });
    }
    ERROR_SUCCESS
}

#[win32_derive::dllexport]
pub fn RegOpenKeyExA(
    ctx: &mut Context,
    hKey: u32,
    lpSubKey: Ptr<u8>,
    _ulOptions: u32,
    _samDesired: u32,
    phkResult: Ptr<u32>,
) -> u32 {
    let sub_key = read_name(ctx, &lpSubKey);
    let handle = with(|state| {
        let path = state.path(hKey, &sub_key);
        state
            .keys
            .contains_key(&path)
            .then(|| state.open_handle(path))
    });
    match handle {
        Some(handle) => {
            phkResult.write(&mut ctx.memory, handle);
            ERROR_SUCCESS
        }
        None => ERROR_FILE_NOT_FOUND,
    }
}

#[win32_derive::dllexport]
pub fn RegCloseKey(_ctx: &mut Context, hKey: u32) -> u32 {
    with(|state| state.open.remove(&hKey));
    ERROR_SUCCESS
}

#[win32_derive::dllexport]
pub fn RegQueryValueExA(
    ctx: &mut Context,
    hKey: u32,
    lpValueName: Ptr<u8>,
    _lpReserved: u32,
    lpType: Ptr<u32>,
    lpData: Ptr<u8>,
    lpcbData: Ptr<u32>,
) -> u32 {
    let name = read_name(ctx, &lpValueName).to_lowercase();
    let found = with(|state| {
        let path = state.open.get(&hKey)?;
        let value = state.keys.get(path)?.get(&name)?;
        Some((value.kind, value.data.clone()))
    });
    let Some((kind, data)) = found else {
        return ERROR_FILE_NOT_FOUND;
    };
    if lpType.addr != 0 {
        lpType.write(&mut ctx.memory, kind);
    }
    if lpcbData.addr == 0 {
        return ERROR_SUCCESS;
    }
    let capacity = lpcbData.read(&ctx.memory).unwrap_or(0) as usize;
    lpcbData.write(&mut ctx.memory, data.len() as u32);
    if lpData.addr == 0 {
        return ERROR_SUCCESS;
    }
    if capacity < data.len() {
        return ERROR_MORE_DATA;
    }
    ctx.memory[lpData.addr..][..data.len()].copy_from_slice(&data);
    ERROR_SUCCESS
}

#[win32_derive::dllexport]
pub fn RegSetValueExA(
    ctx: &mut Context,
    hKey: u32,
    lpValueName: Ptr<u8>,
    _Reserved: u32,
    dwType: u32,
    lpData: Ptr<u8>,
    cbData: u32,
) -> u32 {
    let name = read_name(ctx, &lpValueName).to_lowercase();
    let data = ctx.memory[lpData.addr..][..cbData as usize].to_vec();
    with(|state| {
        let Some(path) = state.open.get(&hKey).cloned() else {
            return ERROR_FILE_NOT_FOUND;
        };
        let values = state.keys.entry(path).or_default();
        let unchanged = values
            .get(&name)
            .is_some_and(|v| v.kind == dwType && v.data == data);
        if !unchanged {
            values.insert(name, Value { kind: dwType, data });
            state.save();
        }
        ERROR_SUCCESS
    })
}
