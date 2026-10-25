//! MCP stdio transport with a visible incoming-line cap (#4295).
//!
//! rmcp's `AsyncRwTransport` has its own line cap, but on an over-cap line it
//! logs, drops the line and keeps the connection open: the client's request
//! vanishes with no reply and no close. This module puts a reader in front of
//! it that turns an over-cap line into an `io::Error`. rmcp treats a read error
//! as the end of the stream, so the session ends and the peer sees EOF.
//!
//! The cap here counts a line's bytes including its terminating `\n`, the same
//! measure rmcp uses, and rmcp's own cap is set to the same value, so a line
//! this reader admits is never one rmcp's silent cap would drop.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use rmcp::RoleServer;
use rmcp::transport::async_rw::AsyncRwTransport;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Longest incoming line, in bytes including the newline, the host accepts on
/// its MCP transport (#4295).
///
/// Equal to rmcp's `DEFAULT_MAX_LINE_LENGTH` (16 MiB), so accepted traffic is
/// unchanged; what #4295 changes is the outcome above the cap: the stream is
/// closed instead of the line being silently dropped.
pub const MAX_INCOMING_LINE_BYTES: usize = 16 * 1024 * 1024;

/// An `AsyncRead` that fails once any line exceeds its cap.
///
/// The failure is sticky: after it, every read returns the same error kind, so
/// a consumer that retries cannot resume mid-line. At most one read buffer past
/// the cap is ever pulled from the inner reader.
pub struct LineCappedReader<R> {
    inner: R,
    cap: usize,
    /// Bytes of the current line seen so far, newline included.
    line_len: usize,
    exceeded: bool,
}

impl<R> LineCappedReader<R> {
    /// Wrap `inner`; any line longer than `cap` bytes, newline included, fails
    /// the read.
    pub fn new(inner: R, cap: usize) -> Self {
        Self {
            inner,
            cap,
            line_len: 0,
            exceeded: false,
        }
    }

    fn exceeded_error(&self) -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("incoming line exceeds the {} byte cap", self.cap),
        )
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for LineCappedReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.exceeded {
            return Poll::Ready(Err(self.exceeded_error()));
        }
        let before = buf.filled().len();
        match Pin::new(&mut self.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {}
            other => return other,
        }
        let mut line_len = self.line_len;
        let mut over = false;
        for &byte in &buf.filled()[before..] {
            line_len += 1;
            if line_len > self.cap {
                over = true;
                break;
            }
            if byte == b'\n' {
                line_len = 0;
            }
        }
        self.line_len = line_len;
        if over {
            self.exceeded = true;
            // Hand back none of the bytes of the failed read.
            buf.set_filled(before);
            return Poll::Ready(Err(self.exceeded_error()));
        }
        Poll::Ready(Ok(()))
    }
}

/// The server transport over `read`/`write` with `cap` applied to incoming lines.
///
/// The one transport constructor for production and tests, so a test with a
/// small `cap` drives the same code path as the host. rmcp's own line cap is set
/// to the same `cap` rather than left at its default, so the two caps agree by
/// construction; [`LineCappedReader`] fires first and makes the failure visible.
pub fn capped_transport<R, W>(
    read: R,
    write: W,
    cap: usize,
) -> AsyncRwTransport<RoleServer, LineCappedReader<R>, W>
where
    R: AsyncRead + Send + Unpin,
    W: AsyncWrite + Send + Unpin + 'static,
{
    AsyncRwTransport::new_server(LineCappedReader::new(read, cap), write).with_max_line_length(cap)
}

/// The host's stdio transport: stdin/stdout capped at [`MAX_INCOMING_LINE_BYTES`].
pub fn stdio_transport()
-> AsyncRwTransport<RoleServer, LineCappedReader<tokio::io::Stdin>, tokio::io::Stdout> {
    capped_transport(
        tokio::io::stdin(),
        tokio::io::stdout(),
        MAX_INCOMING_LINE_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    /// A reader that yields 1, 2, ... up to `max_chunk` bytes per poll, then
    /// starts over, so line boundaries and the cap fall across polls.
    struct Trickle {
        data: Vec<u8>,
        pos: usize,
        next_chunk: usize,
        max_chunk: usize,
    }

    impl Trickle {
        fn new(data: &[u8], max_chunk: usize) -> Self {
            Self {
                data: data.to_vec(),
                pos: 0,
                next_chunk: 1,
                max_chunk,
            }
        }
    }

    impl AsyncRead for Trickle {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            let n = self
                .next_chunk
                .min(self.data.len() - self.pos)
                .min(buf.remaining());
            let start = self.pos;
            buf.put_slice(&self.data[start..start + n]);
            self.pos += n;
            self.next_chunk = if self.next_chunk >= self.max_chunk {
                1
            } else {
                self.next_chunk + 1
            };
            Poll::Ready(Ok(()))
        }
    }

    /// Read to EOF through a capped reader over a trickle of `data`.
    async fn read_through(data: &[u8], cap: usize, max_chunk: usize) -> io::Result<Vec<u8>> {
        let mut reader = LineCappedReader::new(Trickle::new(data, max_chunk), cap);
        let mut out = Vec::new();
        reader.read_to_end(&mut out).await?;
        Ok(out)
    }

    fn line(content_len: usize, ending: &[u8]) -> Vec<u8> {
        let mut line = vec![b'a'; content_len];
        line.extend_from_slice(ending);
        line
    }

    #[tokio::test]
    async fn two_consecutive_lines_each_exactly_at_cap_are_served() {
        let mut data = line(9, b"\n");
        data.extend(line(9, b"\n"));
        for max_chunk in 1..=3 {
            let out = read_through(&data, 10, max_chunk).await.expect("served");
            assert_eq!(out, data, "chunk size {max_chunk}");
        }
    }

    #[tokio::test]
    async fn line_one_byte_over_cap_fails() {
        for max_chunk in 1..=3 {
            let err = read_through(&line(10, b"\n"), 10, max_chunk)
                .await
                .expect_err("cap + 1 must fail");
            assert_eq!(err.kind(), io::ErrorKind::InvalidData, "chunk {max_chunk}");
        }
    }

    #[tokio::test]
    async fn over_cap_second_line_fails_after_a_good_first_line() {
        let mut data = line(9, b"\n");
        data.extend(line(10, b"\n"));
        for max_chunk in 1..=3 {
            let err = read_through(&data, 10, max_chunk)
                .await
                .expect_err("second line is over the cap");
            assert_eq!(err.kind(), io::ErrorKind::InvalidData, "chunk {max_chunk}");
        }
    }

    #[tokio::test]
    async fn crlf_counts_both_bytes() {
        for max_chunk in 1..=3 {
            let at_cap = line(8, b"\r\n");
            assert_eq!(
                read_through(&at_cap, 10, max_chunk).await.expect("served"),
                at_cap
            );
            assert!(
                read_through(&line(9, b"\r\n"), 10, max_chunk)
                    .await
                    .is_err(),
                "9 + CRLF = 11 bytes must fail at cap 10 (chunk {max_chunk})"
            );
        }
    }

    #[tokio::test]
    async fn unterminated_line_over_cap_fails_without_a_newline() {
        for max_chunk in 1..=3 {
            assert!(read_through(&line(11, b""), 10, max_chunk).await.is_err());
            assert_eq!(
                read_through(&line(10, b""), 10, max_chunk)
                    .await
                    .expect("served"),
                line(10, b"")
            );
        }
    }

    #[tokio::test]
    async fn failure_is_sticky() {
        let mut reader = LineCappedReader::new(Trickle::new(&line(20, b"\n"), 3), 10);
        let mut buf = [0u8; 8];
        let first = loop {
            match reader.read(&mut buf).await {
                Ok(0) => panic!("hit EOF before the cap fired"),
                Ok(_) => continue,
                Err(e) => break e,
            }
        };
        assert_eq!(first.kind(), io::ErrorKind::InvalidData);
        for _ in 0..3 {
            let again = reader.read(&mut buf).await.expect_err("stays failed");
            assert_eq!(again.kind(), io::ErrorKind::InvalidData);
        }
    }
}
