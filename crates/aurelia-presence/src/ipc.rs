//! Windows Discord IPC transport: length-prefixed JSON over the local named pipe.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::raw::c_void;
use std::os::windows::io::AsRawHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn PeekNamedPipe(
        pipe: *mut c_void,
        buffer: *mut c_void,
        buffer_size: u32,
        bytes_read: *mut u32,
        total_bytes_avail: *mut u32,
        bytes_left: *mut u32,
    ) -> i32;
}

pub struct DiscordIpc {
    pipe: File,
}

impl DiscordIpc {
    pub fn connect(client_id: &str) -> io::Result<Self> {
        let mut last_error = None;
        for index in 0..10 {
            let path = format!(r"\\.\pipe\discord-ipc-{index}");
            match OpenOptions::new().read(true).write(true).open(&path) {
                Ok(pipe) => {
                    let mut ipc = Self { pipe };
                    ipc.write_frame(0, &json!({ "v": 1, "client_id": client_id }))?;
                    // Discord drops frames sent before it emits READY. Wait for
                    // the dispatch frame before the caller sends SET_ACTIVITY.
                    ipc.wait_for_ready()?;
                    return Ok(ipc);
                }
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "Discord IPC pipe not found")
        }))
    }

    pub fn set_activity(&mut self, activity: &Value) -> io::Result<()> {
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": std::process::id(),
                "activity": activity
            },
            "nonce": nonce()
        });
        self.write_frame(1, &payload)
    }

    pub fn clear(&mut self) -> io::Result<()> {
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": std::process::id(),
                "activity": Value::Null
            },
            "nonce": nonce()
        });
        self.write_frame(1, &payload)
    }

    /// Read any frames Discord has sent and answer PING with PONG.
    ///
    /// Discord sends periodic PING frames; a client that never reads the pipe
    /// can be disconnected after the ping timeout, which silently clears the
    /// activity from the profile.
    pub fn poll(&mut self) -> io::Result<()> {
        loop {
            if self.available()? < 8 {
                return Ok(());
            }
            let mut header = [0u8; 8];
            self.pipe.read_exact(&mut header)?;
            let opcode = u32::from_le_bytes(header[0..4].try_into().unwrap());
            let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
            if length > 1_000_000 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Discord IPC frame too large",
                ));
            }
            let mut body = vec![0u8; length];
            self.pipe.read_exact(&mut body)?;
            if opcode == 3 {
                // PING -> PONG, echoing the raw payload.
                self.pipe.write_all(&frame_bytes(4, &body))?;
                self.pipe.flush()?;
            }
        }
    }

    /// Consume Discord's READY dispatch before the caller sends activity.
    fn wait_for_ready(&mut self) -> io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.available()? < 8 {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Discord did not send READY",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut header = [0u8; 8];
        self.pipe.read_exact(&mut header)?;
        let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let mut body = vec![0u8; length];
        self.pipe.read_exact(&mut body)?;
        Ok(())
    }

    fn available(&self) -> io::Result<u32> {
        let mut available = 0u32;
        let ok = unsafe {
            PeekNamedPipe(
                self.pipe.as_raw_handle() as *mut c_void,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(available)
    }

    fn write_frame(&mut self, opcode: u32, value: &Value) -> io::Result<()> {
        let body = serde_json::to_vec(value)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        self.pipe.write_all(&frame_bytes(opcode, &body))?;
        self.pipe.flush()
    }
}

fn nonce() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{}-{}", std::process::id(), millis)
}

pub(crate) fn frame_bytes(opcode: u32, body: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(8 + body.len());
    frame.extend_from_slice(&opcode.to_le_bytes());
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(body);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_use_little_endian_opcode_and_length() {
        let body = b"{\"cmd\":\"x\"}";
        let frame = frame_bytes(1, body);
        assert_eq!(&frame[0..4], &1u32.to_le_bytes());
        assert_eq!(&frame[4..8], &(body.len() as u32).to_le_bytes());
        assert_eq!(&frame[8..], body);
    }
}
