//! Bounded unary RPC for Lively's local CommandsService. Protocol references:
//! docs/lively-control-protocol.md. No child process or fire-and-forget dispatch.
use anyhow::{Context, Result, bail};
use std::time::{Duration, Instant};
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_IO_PENDING, ERROR_MORE_DATA, ERROR_PIPE_BUSY,
            HANDLE, WAIT_OBJECT_0,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_MODE, OPEN_EXISTING, ReadFile, WriteFile,
        },
        System::{
            IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
            Pipes::{PIPE_READMODE_MESSAGE, SetNamedPipeHandleState},
            Threading::{CreateEventW, WaitForSingleObject},
        },
    },
    core::PCWSTR,
};

const LIMIT: usize = 64 * 1024;
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
fn varint(out: &mut Vec<u8>, mut n: usize) {
    while n >= 128 {
        out.push((n as u8 & 127) | 128);
        n >>= 7;
    }
    out.push(n as u8);
}
fn field(out: &mut Vec<u8>, number: usize, value: &[u8]) {
    varint(out, number * 8 + 2);
    varint(out, value.len());
    out.extend_from_slice(value);
}
fn envelope(out: &mut Vec<u8>, number: usize, value: &[u8]) {
    let mut message = Vec::new();
    field(&mut message, number, value);
    varint(out, message.len());
    out.extend(message);
}
fn request(method: &str, payload: &[u8]) -> Result<Vec<u8>> {
    if payload.len() > 8192 {
        bail!("壁纸控制请求过大。");
    }
    let mut init = Vec::new();
    field(
        &mut init,
        1,
        format!("/Lively.Grpc.Common.Proto.Commands.CommandsService/{method}").as_bytes(),
    );
    let mut out = Vec::new();
    envelope(&mut out, 1, &init);
    envelope(&mut out, 2, &[]);
    let mut info = vec![8];
    varint(&mut info, payload.len());
    info.extend([16, 1]);
    envelope(&mut out, 3, &info);
    out.extend_from_slice(payload);
    Ok(out)
}
struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn integer(&mut self) -> Result<usize> {
        let mut value = 0usize;
        for shift in (0..64).step_by(7) {
            let byte = *self.0.first().context("壁纸响应不完整。")?;
            self.0 = &self.0[1..];
            if shift == 63 && byte > 1 {
                bail!("壁纸响应整数溢出。");
            }
            value |= usize::from(byte & 127) << shift;
            if byte & 128 == 0 {
                return Ok(value);
            }
        }
        bail!("壁纸响应整数无效。")
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if len > LIMIT || len > self.0.len() {
            bail!("壁纸响应长度无效。");
        }
        let (value, tail) = self.0.split_at(len);
        self.0 = tail;
        Ok(value)
    }
    fn blob(&mut self) -> Result<&'a [u8]> {
        let size = self.integer()?;
        self.take(size)
    }
    fn skip(&mut self, wire: usize) -> Result<()> {
        match wire {
            0 => {
                self.integer()?;
            }
            2 => {
                self.blob()?;
            }
            _ => bail!("壁纸响应字段无效。"),
        }
        Ok(())
    }
}
fn response(packet: &[u8]) -> Result<bool> {
    let mut reader = Reader(packet);
    while !reader.0.is_empty() {
        let mut message = Reader(reader.blob()?);
        let key = message.integer()?;
        if key & 7 != 2 {
            bail!("壁纸响应类型无效。");
        }
        let body = message.blob()?;
        if !message.0.is_empty() {
            bail!("壁纸响应封装无效。");
        }
        match key >> 3 {
            2 => (), // Response headers.
            3 => {
                let mut info = Reader(body);
                let mut size = 0;
                let mut inline = false;
                while !info.0.is_empty() {
                    let key = info.integer()?;
                    match key {
                        8 => size = info.integer()?,
                        16 => inline = info.integer()? == 1,
                        _ => info.skip(key & 7)?,
                    }
                }
                // These unary methods return protobuf Empty; unexpected payloads
                // fail closed instead of being mistaken for an acknowledgement.
                if size != 0 || !inline {
                    bail!("壁纸组件返回了不兼容的响应。");
                }
            }
            5 => {
                let mut trailers = Reader(body);
                let mut status = 0;
                let mut detail = String::new();
                while !trailers.0.is_empty() {
                    let key = trailers.integer()?;
                    match key {
                        16 => status = trailers.integer()?,
                        26 => {
                            detail = String::from_utf8_lossy(trailers.blob()?)
                                .chars()
                                .take(256)
                                .collect()
                        }
                        _ => trailers.skip(key & 7)?,
                    }
                }
                if status != 0 {
                    bail!("壁纸组件拒绝操作（{status}）：{detail}");
                }
                if !reader.0.is_empty() {
                    bail!("壁纸确认后包含意外数据。");
                }
                return Ok(true);
            }
            _ => bail!("壁纸组件响应不兼容。"),
        }
    }
    Ok(false)
}
fn connect(deadline: Instant) -> Result<Handle> {
    let username = std::env::var("USERNAME").context("无法读取壁纸会话用户。");
    let username = username?;
    if username.is_empty() || username.len() > 256 || username.contains(['\\', '/', '\0']) {
        bail!("壁纸会话用户无效。");
    }
    let path: Vec<u16> = format!(r"\\.\pipe\Grpc_LIVELY:DESKTOPWALLPAPERSYSTEM{username}")
        .encode_utf16()
        .chain([0])
        .collect();
    loop {
        let file = unsafe {
            CreateFileW(
                PCWSTR(path.as_ptr()),
                0xc0000000,
                FILE_SHARE_MODE(0),
                None,
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                None,
            )
        };
        match file {
            Ok(file) => {
                let file = Handle(file);
                unsafe {
                    SetNamedPipeHandleState(file.0, Some(&PIPE_READMODE_MESSAGE), None, None)
                }?;
                return Ok(file);
            }
            Err(error)
                if [
                    ERROR_FILE_NOT_FOUND.to_hresult(),
                    ERROR_PIPE_BUSY.to_hresult(),
                ]
                .contains(&error.code()) =>
            {
                if Instant::now() >= deadline {
                    bail!("动态壁纸控制接口未就绪，请稍后重试。");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(error).context("无法连接当前用户的壁纸控制接口。"),
        }
    }
}
fn transfer(
    file: HANDLE,
    read: Option<&mut [u8]>,
    write: Option<&[u8]>,
    deadline: Instant,
) -> Result<(usize, bool)> {
    let event = Handle(unsafe { CreateEventW(None, true, false, None) }?);
    let mut overlapped = OVERLAPPED {
        hEvent: event.0,
        ..Default::default()
    };
    let mut count = 0u32;
    let result = unsafe {
        if let Some(buffer) = read {
            ReadFile(file, Some(buffer), Some(&mut count), Some(&mut overlapped))
        } else {
            WriteFile(file, write, Some(&mut count), Some(&mut overlapped))
        }
    };
    let result = match result {
        Err(error) if error.code() == ERROR_IO_PENDING.to_hresult() => {
            let remaining = deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(15_000) as u32;
            if unsafe { WaitForSingleObject(event.0, remaining) } != WAIT_OBJECT_0 {
                // Join cancellation before dropping the buffer/OVERLAPPED.
                unsafe {
                    let _ = CancelIoEx(file, Some(&overlapped));
                    let _ = GetOverlappedResult(file, &overlapped, &mut count, true);
                }
                bail!("动态壁纸控制超时，未确认操作成功。");
            }
            unsafe { GetOverlappedResult(file, &overlapped, &mut count, false) }
        }
        other => other,
    };
    match result {
        Ok(()) => Ok((count as usize, false)),
        Err(error) if error.code() == ERROR_MORE_DATA.to_hresult() => Ok((count as usize, true)),
        Err(error) => Err(error).context("动态壁纸控制连接中断，未确认操作成功。"),
    }
}
fn call(method: &str, payload: &[u8]) -> Result<()> {
    let packet = request(method, payload)?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let file = connect(Instant::now() + Duration::from_secs(8))?;
    let (count, _) = transfer(file.0, None, Some(&packet), deadline)?;
    if count != packet.len() {
        bail!("壁纸控制请求未完整发送。");
    }
    for _ in 0..16 {
        let mut packet = Vec::new();
        loop {
            let mut chunk = [0u8; 16 * 1024];
            let (count, more) = transfer(file.0, Some(&mut chunk), None, deadline)?;
            if count == 0 || packet.len() + count > LIMIT {
                bail!("壁纸控制响应大小无效。");
            }
            packet.extend_from_slice(&chunk[..count]);
            if !more {
                break;
            }
        }
        if response(&packet)? {
            return Ok(());
        }
    }
    bail!("壁纸组件未确认操作结果。")
}
pub fn command(args: &[String]) -> Result<()> {
    let mut payload = Vec::new();
    for arg in args {
        field(&mut payload, 1, arg.as_bytes());
    }
    call("AutomationCommand", &payload)
}
pub fn shutdown() -> Result<()> {
    call("ShutDown", &[])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acknowledgement_requires_valid_trailers() {
        assert!(!response(&[2, 18, 0]).unwrap());
        assert!(response(&[2, 42, 0]).unwrap());
        assert!(response(&[4, 42, 2, 16, 13]).is_err());
        for bytes in [&[128][..], &[5, 42, 0], &[2, 10, 0], &[2, 42, 0, 0]] {
            assert!(response(bytes).is_err());
        }
    }
    #[test]
    fn supports_utf8_and_protobuf_length_boundaries() {
        let mut payload = Vec::new();
        field(&mut payload, 1, "C:\\壁纸\\index.html".as_bytes());
        let packet = request("AutomationCommand", &payload).unwrap();
        assert!(packet.ends_with(&payload));
        assert!(request("AutomationCommand", &vec![0; 8193]).is_err());
        for value in [0, 127, 128, 16384, usize::MAX] {
            let mut bytes = Vec::new();
            varint(&mut bytes, value);
            assert_eq!(Reader(&bytes).integer().unwrap(), value);
        }
    }
}
