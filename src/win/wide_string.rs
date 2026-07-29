use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

use windows::core::PCWSTR;

const EMPTY_STRING_ARRAY: [u16; 1] = [0];

#[derive(Debug, Clone)]
pub struct WideString {
    inner: Vec<u16>,
}

impl WideString {
    pub fn new(s: &str) -> Self {
        Self::from_os_string(OsStr::new(s))
    }

    pub fn from_os_string(s: &OsStr) -> Self {
        Self {
            inner: s.encode_wide().chain(std::iter::once(0)).collect(),
        }
    }

    #[allow(dead_code)]
    pub fn from_pcwstr(s: PCWSTR) -> Self {
        let mut len = 0;
        unsafe {
            while *s.0.add(len) != 0 {
                len += 1;
            }
        }
        let mut v = vec![0u16; len + 1];
        unsafe {
            std::ptr::copy_nonoverlapping(s.0, v.as_mut_ptr(), len + 1);
        }
        Self { inner: v }
    }

    #[allow(dead_code)]
    pub fn from_vec_u16(mut v: Vec<u16>) -> Self {
        if v.len() == 0 {
            return Self::empty();
        }
        let last_index = v.len() - 1;
        if v[last_index] != 0 {
            v.push(0);
        }
        Self { inner: v }
    }

    #[allow(dead_code)]
    pub fn null() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn empty() -> Self {
        Self {
            inner: EMPTY_STRING_ARRAY.to_vec(),
        }
    }

    pub fn as_pcwstr(&self) -> PCWSTR {
        if self.inner.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(self.inner.as_ptr())
        }
    }

    #[allow(dead_code)]
    pub fn to_string(&self) -> Result<Option<String>, std::string::FromUtf16Error> {
        let p = self.as_pcwstr();
        if p.is_null() {
            Ok(None)
        } else {
            let s = unsafe { p.to_string() }?;
            Ok(Some(s))
        }
    }

    #[allow(dead_code)]
    pub fn clone_u16_array(&self) -> Vec<u16> {
        self.inner.clone()
    }

    // pub fn wstr_copy(src: PCWSTR, dst: PWSTR, max_chars: usize) -> Result<usize, String> {
    //     if src.is_null() {
    //         return Err("Null source pointer".to_string());
    //     }
    //     if dst.is_null() {
    //         return Err("Null destination pointer".to_string());
    //     }
    //     let p_dst = dst.as_ptr();
    //     let count = WideStringIterator::new(src)
    //         .take(max_chars)
    //         .enumerate()
    //         .inspect(|(i, c)| unsafe { *p_dst.add(*i) = *c; })
    //     .count();
    //     dst
    //     Ok(count)
    // }

    pub fn copy_to(&self, dst: &mut [u16]) -> usize {
        let count = self
            .inner
            .iter()
            .take(dst.len() - 1)
            .enumerate()
            .inspect(|(i, c)| dst[*i] = **c)
            .count();
        dst[count] = 0;
        count
    }
}

#[allow(dead_code)]
struct WideStringIterator {
    ptr: *const u16,
}

impl WideStringIterator {
    #[allow(dead_code)]
    fn new(ptr: PCWSTR) -> Self {
        Self { ptr: ptr.as_ptr() }
    }
}

impl Iterator for WideStringIterator {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let ch = *self.ptr;
            if ch == 0 {
                return None;
            }
            self.ptr = self.ptr.add(1);
            return Some(ch);
        }
    }
}
