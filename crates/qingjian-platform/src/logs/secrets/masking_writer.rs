use std::borrow::Cow;
use std::io::{self, Write};

use super::mask;

/// 包在日志文件外面的写入器：每次写入先过一遍 [`mask`]。
/// 要放在 `tracing_appender::non_blocking` 里面——那里一条日志是一次完整的写入，密钥不会被切成两半。
pub struct MaskingWriter<W> {
    inner: W,
}

impl<W> MaskingWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for MaskingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match mask(&String::from_utf8_lossy(buf)) {
            Cow::Borrowed(_) => self.inner.write_all(buf)?,
            Cow::Owned(masked) => self.inner.write_all(masked.as_bytes())?,
        }
        // 写进去的字节数可能变少了，对调用方仍按原长度算写完
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
