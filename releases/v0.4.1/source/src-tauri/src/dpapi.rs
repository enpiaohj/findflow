use windows::core::PCWSTR;
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN,
};

/// DPAPI CurrentUser 加密（设计文档 §10.3：API Key 不写入普通配置文件）。
pub fn protect(plain: &[u8]) -> Result<Vec<u8>, String> {
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: plain.len() as u32,
        pbData: plain.as_ptr() as *mut u8,
    };
    let mut out = CRYPT_INTEGER_BLOB::default();
    let hr = unsafe {
        CryptProtectData(
            &in_blob,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )
    };
    if hr.is_err() {
        return Err("DPAPI 加密失败".into());
    }
    let bytes = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
    unsafe { windows::Win32::System::Com::CoTaskMemFree(Some(out.pbData as *const _)) };
    Ok(bytes)
}

pub fn unprotect(encrypted: &[u8]) -> Result<Vec<u8>, String> {
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: encrypted.len() as u32,
        pbData: encrypted.as_ptr() as *mut u8,
    };
    let mut out = CRYPT_INTEGER_BLOB::default();
    let hr = unsafe {
        CryptUnprotectData(
            &in_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )
    };
    if hr.is_err() {
        return Err("DPAPI 解密失败（可能来自其他用户或已损坏）".into());
    }
    let bytes = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
    unsafe { windows::Win32::System::Com::CoTaskMemFree(Some(out.pbData as *const _)) };
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protect_roundtrip() {
        let secret = b"findflow-test-secret";
        let enc = protect(secret).unwrap();
        assert_ne!(enc, secret.to_vec());
        let dec = unprotect(&enc).unwrap();
        assert_eq!(dec, secret.to_vec());
    }
}
