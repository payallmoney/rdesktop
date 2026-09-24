// 注册表持久化:HKCU\Software\rdesktop
// 面板位置/大小/状态、全局设置、条目分组映射与手动顺序,全部存这里(用户级,无需管理员)。
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_DWORD, REG_MULTI_SZ,
    REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_MULTI_SZ, RRF_RT_REG_SZ,
};

use crate::app::ws;

const KEY: &str = "Software\\rdesktop";

fn parts(name: &str) -> (Vec<u16>, Vec<u16>) {
    (ws(KEY), ws(name))
}

pub fn set_dw(name: &str, v: u32) {
    let (k, n) = parts(name);
    let data = v.to_le_bytes();
    unsafe {
        let r = RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            REG_DWORD.0,
            Some(data.as_ptr() as *const _),
            4,
        );
        if r.0 != 0 {
            crate::panel::dlog(&format!("reg set {name} FAIL {:?}", r));
        }
    }
}

pub fn get_dw(name: &str) -> Option<u32> {
    let (k, n) = parts(name);
    let mut out = [0u8; 4];
    let mut cb = 4u32;
    unsafe {
        let r = RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(out.as_mut_ptr() as *mut _),
            Some(&mut cb),
        );
        if r.0 == 0 && cb == 4 {
            Some(u32::from_le_bytes(out))
        } else {
            None
        }
    }
}

pub fn set_sz(name: &str, v: &str) {
    let (k, n) = parts(name);
    let mut data = ws(v);
    unsafe {
        let r = RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            REG_SZ.0,
            Some(data.as_ptr() as *const _),
            (data.len() * 2) as u32,
        );
        if r.0 != 0 {
            crate::panel::dlog(&format!("reg set {name} FAIL {:?}", r));
        }
    }
}

pub fn get_sz(name: &str) -> Option<String> {
    let (k, n) = parts(name);
    let mut buf = [0u16; 2048];
    let mut cb = (buf.len() * 2) as u32;
    unsafe {
        let r = RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut cb),
        );
        if r.0 != 0 {
            return None;
        }
        let len = (cb as usize) / 2;
        let end = buf[..len].iter().position(|&c| c == 0).unwrap_or(len);
        Some(String::from_utf16_lossy(&buf[..end]))
    }
}

/// REG_MULTI_SZ 写入(空列表 = 仅终止符)
pub fn set_multi(name: &str, items: &[String]) {
    let (k, n) = parts(name);
    let mut data: Vec<u16> = Vec::new();
    for it in items {
        data.extend(ws(it));
    }
    data.push(0);
    unsafe {
        let r = RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            REG_MULTI_SZ.0,
            Some(data.as_ptr() as *const _),
            (data.len() * 2) as u32,
        );
        if r.0 != 0 {
            crate::panel::dlog(&format!("reg set {name} FAIL {:?}", r));
        }
    }
}

pub fn get_multi(name: &str) -> Vec<String> {
    let (k, n) = parts(name);
    let mut buf = vec![0u16; 65536];
    let mut cb = (buf.len() * 2) as u32;
    unsafe {
        let r = RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_MULTI_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut cb),
        );
        if r.0 != 0 {
            return Vec::new();
        }
        let len = (cb as usize) / 2;
        let mut out: Vec<String> = Vec::new();
        let mut cur = String::new();
        for &c in &buf[..len] {
            if c == 0 {
                if cur.is_empty() {
                    break; // 双 NUL 终止
                }
                out.push(std::mem::take(&mut cur));
            } else {
                cur.push(char::from_u32(c as u32).unwrap_or('\u{FFFD}'));
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out
    }
}

pub fn del(name: &str) {
    let (k, n) = parts(name);
    unsafe {
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr()), PCWSTR(n.as_ptr()));
    }
}
