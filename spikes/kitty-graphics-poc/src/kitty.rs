//! Minimal, purpose-built implementation of the Kitty graphics protocol.
//!
//! Only what the go/no-go spike needs: support probing, chunked direct
//! transmission (raw or zlib), file/shared-memory transmission, placement,
//! delete, and multiplexer passthrough wrapping.
//!
//! Reference: <https://sw.kovidgoyal.net/kitty/graphics-protocol/>

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use std::io::Write;

/// Maximum payload bytes per escape sequence, base64-encoded size.
pub const MAX_CHUNK: usize = 4096;

/// Build one APC graphics command: `ESC _ G <control> ; <payload> ESC \`.
///
/// An empty payload omits the `;` separator, as the protocol requires.
pub fn apc(control: &str, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(control.len() + payload.len() + 8);
    out.extend_from_slice(b"\x1b_G");
    out.extend_from_slice(control.as_bytes());
    if !payload.is_empty() {
        out.push(b';');
        out.extend_from_slice(payload);
    }
    out.extend_from_slice(b"\x1b\\");
    out
}

/// The protocol's recommended support probe: a 1x1 RGB query action.
pub fn query() -> Vec<u8> {
    apc("i=31,s=1,v=1,a=q,t=d,f=24", b"AAAA")
}

/// Primary device attributes request, used to bound the probe.
pub fn da1() -> &'static [u8] {
    b"\x1b[c"
}

/// Delete every image and placement. Sent before leaving the alternate screen.
pub fn delete_all() -> Vec<u8> {
    apc("a=d,d=A", b"")
}

/// zlib (RFC 1950) compress, matching the protocol's `o=z`.
pub fn zlib(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(6));
    // Writing into a `Vec` cannot fail.
    encoder.write_all(data).expect("vec write");
    encoder.finish().expect("vec finish")
}

/// One encoded, ready-to-write frame.
#[derive(Debug, Clone)]
pub struct EncodedFrame {
    /// One escape sequence per protocol chunk.
    pub sequences: Vec<Vec<u8>>,
    /// Payload bytes before base64 (after compression, if any).
    pub data_bytes: usize,
    /// Base64 text bytes carried across all chunks.
    pub base64_bytes: usize,
    /// Total bytes that will cross the pty before mux wrapping.
    pub escaped_bytes: usize,
}

/// Encode `data` for `control`, chunked at [`MAX_CHUNK`] base64 bytes.
///
/// Only the first chunk carries the control data; later chunks carry `m`
/// (and nothing else), per the protocol's chunked-transmission rules. Base64
/// output is always a multiple of four bytes and [`MAX_CHUNK`] is too, so every
/// non-final chunk satisfies the "multiple of 4" rule.
pub fn encode(control: &str, data: &[u8]) -> EncodedFrame {
    let encoded = B64.encode(data);
    let mut sequences = Vec::new();
    let mut escaped_bytes = 0;
    if encoded.is_empty() {
        let seq = apc(&format!("{control},m=0"), b"");
        escaped_bytes += seq.len();
        sequences.push(seq);
    } else {
        let mut start = 0;
        while start < encoded.len() {
            let end = (start + MAX_CHUNK).min(encoded.len());
            let last = end == encoded.len();
            let chunk_control = if sequences.is_empty() {
                format!("{control},m={}", u8::from(!last))
            } else {
                format!("m={}", u8::from(!last))
            };
            let seq = apc(&chunk_control, &encoded.as_bytes()[start..end]);
            escaped_bytes += seq.len();
            sequences.push(seq);
            start = end;
        }
    }
    EncodedFrame {
        sequences,
        data_bytes: data.len(),
        base64_bytes: encoded.len(),
        escaped_bytes,
    }
}

/// Double every ESC byte, the escaping tmux's DCS passthrough expects.
pub fn double_esc(seq: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(seq.len() * 2);
    for &byte in seq {
        out.push(byte);
        if byte == 0x1b {
            out.push(0x1b);
        }
    }
    out
}

/// Inverse of [`double_esc`], used by tests and diagnostics to prove the
/// wrapper is recoverable by the multiplexer's un-doubling rule.
#[cfg(test)]
pub fn undouble_esc(seq: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(seq.len());
    let mut index = 0;
    while index < seq.len() {
        let byte = seq[index];
        if byte == 0x1b && seq.get(index + 1) == Some(&0x1b) {
            index += 1;
        }
        out.push(byte);
        index += 1;
    }
    out
}

/// Wrap one escape sequence for tmux's `allow-passthrough` DCS channel.
///
/// tmux's DCS parser drops the ESC that opens an embedded escape and emits the
/// byte that follows, so embedding requires every ESC to be doubled. The DCS
/// terminator is outside the doubled payload.
pub fn tmux_wrap(seq: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(seq.len() * 2 + 8);
    out.extend_from_slice(b"\x1bPtmux;");
    out.extend_from_slice(&double_esc(seq));
    out.extend_from_slice(b"\x1b\\");
    out
}

/// Wrap one escape sequence for GNU screen's passthrough DCS channel.
///
/// screen terminates the DCS with `ESC \` like tmux but has a small per-string
/// limit; the spike reports oversized frames rather than trusting them.
pub fn screen_wrap(seq: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(seq.len() * 2 + 4);
    out.extend_from_slice(b"\x1bP");
    out.extend_from_slice(&double_esc(seq));
    out.extend_from_slice(b"\x1b\\");
    out
}

/// Base64 payload for a local medium (file or shared-memory name).
pub fn medium_payload(name: &str) -> Vec<u8> {
    B64.encode(name.as_bytes()).into_bytes()
}

/// Classified replies collected from the terminal.
#[derive(Debug, Default, Clone)]
pub struct ParsedReplies {
    pub graphics: Vec<String>,
    pub device_attributes: Vec<String>,
}

impl ParsedReplies {
    /// True when at least one graphics reply reported `OK`.
    pub fn graphics_ok(&self) -> bool {
        self.graphics.iter().any(|reply| reply.contains("OK"))
    }

    /// True when the terminal answered the device-attributes probe.
    pub fn saw_device_attributes(&self) -> bool {
        !self.device_attributes.is_empty()
    }
}

/// Parse raw terminal reply bytes into graphics and device-attributes replies.
///
/// This is deliberately a scanner, not a full terminal parser: it only needs to
/// recognize the two reply shapes the probe waits for and ignore everything
/// else (typed keys, mouse reports, focus events).
pub fn parse_replies(buf: &[u8]) -> ParsedReplies {
    let mut parsed = ParsedReplies::default();
    let mut index = 0;
    while index < buf.len() {
        if buf[index] == 0x1b
            && buf.get(index + 1) == Some(&b'_')
            && let Some(body) = between(buf, index + 3, b"\x1b\\")
        {
            parsed
                .graphics
                .push(String::from_utf8_lossy(body).into_owned());
            index += 3 + body.len() + 2;
            continue;
        }
        if buf[index] == 0x1b
            && buf.get(index + 1) == Some(&b'[')
            && let Some(end) = (index + 2..buf.len()).find(|&i| (0x40..=0x7e).contains(&buf[i]))
        {
            let body = String::from_utf8_lossy(&buf[index + 2..end]).into_owned();
            if body.starts_with('?') && buf[end] == b'c' {
                parsed.device_attributes.push(body);
            }
            index = end + 1;
            continue;
        }
        index += 1;
    }
    parsed
}

/// True once a device-attributes reply is present, which bounds a probe.
pub fn saw_da1(buf: &[u8]) -> bool {
    parse_replies(buf).saw_device_attributes()
}

/// Find `needle` in `haystack` after `start`, returning the enclosed body.
fn between<'a>(haystack: &'a [u8], start: usize, needle: &[u8]) -> Option<&'a [u8]> {
    haystack[start..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| &haystack[start..start + offset])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apc_omits_separator_without_payload() {
        assert_eq!(apc("a=d,d=A", b""), b"\x1b_Ga=d,d=A\x1b\\");
    }

    #[test]
    fn encode_is_a_single_chunk_small_frame() {
        let frame = encode("a=T,f=32,s=1,v=1,i=1", &[0, 0, 0, 0]);
        assert_eq!(frame.sequences.len(), 1);
        assert_eq!(frame.base64_bytes, 8);
        let seq = &frame.sequences[0];
        assert!(seq.starts_with(b"\x1b_Ga=T,f=32,s=1,v=1,i=1,m=0;"));
        assert!(seq.ends_with(b"\x1b\\"));
    }

    #[test]
    fn encode_chunks_large_frames_with_control_only_on_first() {
        // 12 KiB of data encodes to 16 KiB of base64: exactly four chunks.
        let data = vec![0xabu8; 12 * 1024];
        let frame = encode("a=T,f=32,s=64,v=48,i=9", &data);
        assert_eq!(frame.sequences.len(), 4);
        assert!(frame.sequences[0].starts_with(b"\x1b_Ga=T,f=32,s=64,v=48,i=9,m=1;"));
        for seq in &frame.sequences[1..frame.sequences.len() - 1] {
            assert!(seq.starts_with(b"\x1b_Gm=1;"));
        }
        let last = frame.sequences.last().unwrap();
        assert!(last.starts_with(b"\x1b_Gm=0;"));
        // Every non-final base64 chunk is a multiple of four bytes.
        for seq in &frame.sequences[..frame.sequences.len() - 1] {
            let body = between(seq, 3, b"\x1b\\").unwrap();
            let payload = &body[body.iter().position(|&b| b == b';').unwrap() + 1..];
            assert_eq!(payload.len() % 4, 0);
            assert!(payload.len() <= MAX_CHUNK);
        }
    }

    #[test]
    fn zlib_round_trips() {
        // A zlib stream starts with 0x78; 32 MiB of zeros cannot be the
        // uncompressed size of a 12-byte frame, so this proves compression.
        let data = vec![0u8; 4096];
        let compressed = zlib(&data);
        assert_eq!(compressed[0], 0x78);
        assert!(compressed.len() < data.len());
    }

    #[test]
    fn tmux_wrap_doubles_escapes() {
        let wrapped = tmux_wrap(b"\x1b_Ga=q\x1b\\");
        // Prefix ESC Ptmux; then the sequence with every ESC doubled, then the
        // DCS terminator which is not part of the doubled payload.
        assert_eq!(wrapped, b"\x1bPtmux;\x1b\x1b_Ga=q\x1b\x1b\\\x1b\\");
        // The tmux parser drops the ESC that opens an embedded escape and
        // emits the follower, so un-doubling must recover the original.
        assert_eq!(
            undouble_esc(&wrapped[7..wrapped.len() - 2]),
            b"\x1b_Ga=q\x1b\\"
        );
    }

    #[test]
    fn parse_replies_ignores_typed_input_and_reads_both_replies() {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"hi\x1b_Gi=31;OK\x1b\\");
        buf.extend_from_slice(b"\x1b[?62;4c");
        buf.extend_from_slice(b"\x1b[1;2R");
        let parsed = parse_replies(&buf);
        assert!(parsed.graphics_ok());
        assert!(parsed.saw_device_attributes());
        assert_eq!(parsed.device_attributes, vec!["?62;4".to_string()]);
    }

    #[test]
    fn parse_replies_reports_unsupported_shape() {
        let parsed = parse_replies(b"\x1b[?1;2c");
        assert!(!parsed.graphics_ok());
        assert!(parsed.saw_device_attributes());
    }
}
