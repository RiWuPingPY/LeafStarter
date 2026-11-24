//! 令牌加密保存。Windows 用 DPAPI（只有当前 Windows 用户能解开，换台电脑或换个用户就解不开）；
//! 别的系统暂时只做十六进制编码，macOS / Linux 做到时换成钥匙串。

use anyhow::{bail, Result};

pub fn seal(plain: &[u8]) -> Result<String> {
    Ok(to_hex(&protect(plain)?))
}

pub fn open(sealed: &str) -> Result<Vec<u8>> {
    unprotect(&from_hex(sealed)?)
}

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

fn from_hex(s: &str) -> Result<Vec<u8>> {
    if s.len() % 2 != 0 {
        bail!("保存的令牌格式不对");
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| anyhow::anyhow!("保存的令牌格式不对"))).collect()
}

#[cfg(windows)]
mod dpapi {
    #[repr(C)]
    pub struct Blob {
        pub len: u32,
        pub data: *mut u8,
    }
    #[link(name = "crypt32")]
    extern "system" {
        pub fn CryptProtectData(input: *const Blob, desc: *const u16, entropy: *const Blob, reserved: *const u8, prompt: *const u8, flags: u32, out: *mut Blob) -> i32;
        pub fn CryptUnprotectData(input: *const Blob, desc: *mut *mut u16, entropy: *const Blob, reserved: *const u8, prompt: *const u8, flags: u32, out: *mut Blob) -> i32;
    }
    extern "system" {
        pub fn LocalFree(mem: *mut u8) -> *mut u8;
    }
    /// 不弹任何界面
    pub const UI_FORBIDDEN: u32 = 0x1;
    /// 附加熵：只有 LeafStarter 自己解得开（同一用户下别的程序调 DPAPI 也解不开）
    pub const ENTROPY: &[u8] = b"LeafStarter/accounts/v1";
}

#[cfg(windows)]
fn protect(plain: &[u8]) -> Result<Vec<u8>> {
    use dpapi::*;
    let input = Blob { len: plain.len() as u32, data: plain.as_ptr() as *mut u8 };
    let entropy = Blob { len: ENTROPY.len() as u32, data: ENTROPY.as_ptr() as *mut u8 };
    let mut out = Blob { len: 0, data: std::ptr::null_mut() };
    // SAFETY: 输入缓冲区在调用期间有效；输出由系统分配，复制后用 LocalFree 释放
    unsafe {
        if CryptProtectData(&input, std::ptr::null(), &entropy, std::ptr::null(), std::ptr::null(), UI_FORBIDDEN, &mut out) == 0 {
            bail!("加密令牌失败");
        }
        let v = std::slice::from_raw_parts(out.data, out.len as usize).to_vec();
        LocalFree(out.data);
        Ok(v)
    }
}

#[cfg(windows)]
fn unprotect(sealed: &[u8]) -> Result<Vec<u8>> {
    use dpapi::*;
    let input = Blob { len: sealed.len() as u32, data: sealed.as_ptr() as *mut u8 };
    let entropy = Blob { len: ENTROPY.len() as u32, data: ENTROPY.as_ptr() as *mut u8 };
    let mut out = Blob { len: 0, data: std::ptr::null_mut() };
    // SAFETY: 同上
    unsafe {
        if CryptUnprotectData(&input, std::ptr::null_mut(), &entropy, std::ptr::null(), std::ptr::null(), UI_FORBIDDEN, &mut out) == 0 {
            bail!("解不开保存的令牌（换了电脑或 Windows 用户？），重新登录一次就好");
        }
        let v = std::slice::from_raw_parts(out.data, out.len as usize).to_vec();
        LocalFree(out.data);
        Ok(v)
    }
}

#[cfg(not(windows))]
fn protect(plain: &[u8]) -> Result<Vec<u8>> {
    Ok(plain.to_vec())
}

#[cfg(not(windows))]
fn unprotect(sealed: &[u8]) -> Result<Vec<u8>> {
    Ok(sealed.to_vec())
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trip() {
        let s = super::seal(b"refresh-token-123").unwrap();
        assert!(!s.contains("refresh"));
        assert_eq!(super::open(&s).unwrap(), b"refresh-token-123");
        assert!(super::open("zz").is_err());
    }
}
