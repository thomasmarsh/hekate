//! Terminal capability detection and terminal-backend selection.
//!
//! Detection is deliberately conservative. It consults cheap environment hints
//! and then, only for the opt-in backend, the terminal's own Kitty graphics
//! probe bounded by a primary device-attributes request. A reply other than an
//! explicit `OK`, a missing reply, and every ambiguous shape all count as
//! unsupported.
//!
//! A positive probe gates the opt-in Kitty backend but is never proof that the
//! terminal is safe. [[TAS-010-kitty-graphics-poc]] recorded a terminal that
//! answered the probe `OK` and then crashed under the naive same-id lifecycle,
//! so selection only ever chooses a backend that uses the bounded lifecycle
//! mandated by [[DEC-002-terminal-backend-strategy]]; no probe result selects a
//! naive transmission path.

use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

/// Kitty graphics support query, followed by a primary device-attributes
/// request (`CSI c`) that bounds the wait.
///
/// The graphics query is a 1x1 RGB query action (`a=q`), the probe recommended
/// by the protocol. The payload `AAAA` is four bytes of base64-decoded zeroes,
/// which is all a query action needs.
pub const PROBE_QUERY: &[u8] = b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c";

/// Extra time allowed for a graphics reply that races the device-attributes
/// reply that ends the probe.
const REPLY_GRACE: Duration = Duration::from_millis(40);

/// Environment variables a capability decision may consult.
pub const HINT_KEYS: &[&str] = &[
    "TERM",
    "COLORTERM",
    "TERM_PROGRAM",
    "TERM_PROGRAM_VERSION",
    "KITTY_WINDOW_ID",
    "GHOSTTY_RESOURCES_DIR",
    "WEZTERM_EXECUTABLE",
    "KONSOLE_VERSION",
    "TMUX",
    "STY",
    "ZELLIJ",
    "SSH_CONNECTION",
];

/// Which terminal backend the user asked for on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendRequest {
    /// Character cells only; never probe the terminal.
    Ascii,
    /// Kitty graphics with the bounded pair lifecycle; never probe.
    Kitty,
    /// Probe the terminal and fall back to character cells.
    #[default]
    Auto,
}

impl BackendRequest {
    /// Parse the `--backend` flag value.
    pub fn from_flag(value: &str) -> Option<Self> {
        match value {
            "ascii" => Some(Self::Ascii),
            "kitty" => Some(Self::Kitty),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }

    /// The flag spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ascii => "ascii",
            Self::Kitty => "kitty",
            Self::Auto => "auto",
        }
    }
}

/// Which terminal backend a selection resolved to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendKind {
    /// The universal character-cell backend.
    #[default]
    Ascii,
    /// The opt-in Kitty graphics backend with the bounded pair lifecycle.
    Kitty,
}

impl BackendKind {
    /// Resolve a request against a probe verdict.
    ///
    /// An explicit request overrides detection. `auto` selects Kitty only for
    /// an explicit `Supported` verdict: every other shape, including a missing
    /// or ambiguous reply, is treated as unsupported.
    pub const fn resolve(request: BackendRequest, verdict: CapabilityVerdict) -> Self {
        match request {
            BackendRequest::Ascii => Self::Ascii,
            BackendRequest::Kitty => Self::Kitty,
            BackendRequest::Auto if verdict.is_supported() => Self::Kitty,
            BackendRequest::Auto => Self::Ascii,
        }
    }

    /// The backend spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ascii => "ascii",
            Self::Kitty => "kitty",
        }
    }
}

/// What a capability probe observed about the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CapabilityVerdict {
    /// The terminal answered the graphics probe `OK`.
    Supported,
    /// A device-attributes reply arrived with no graphics reply, or the
    /// environment hints definitively ruled graphics out.
    #[default]
    Unsupported,
    /// The terminal answered the graphics probe with an error.
    GraphicsError,
    /// Nothing answered before the timeout.
    NoReply,
}

impl CapabilityVerdict {
    /// Whether the verdict is the single positive signal.
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }

    /// The verdict spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
            Self::GraphicsError => "graphics-error",
            Self::NoReply => "no-reply",
        }
    }
}

/// Cheap, spoofable environment hints about the terminal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvironmentHints {
    hints: Vec<(&'static str, String)>,
}

impl EnvironmentHints {
    /// Capture the process's current environment.
    pub fn capture() -> Self {
        Self {
            hints: HINT_KEYS
                .iter()
                .filter_map(|key| std::env::var(key).ok().map(|value| (*key, value)))
                .collect(),
        }
    }

    /// Build hints from explicit pairs, for tests and callers that already hold
    /// the environment.
    pub fn from_pairs(pairs: impl IntoIterator<Item = (&'static str, String)>) -> Self {
        Self {
            hints: pairs.into_iter().collect(),
        }
    }

    /// The value of one hint, if it was set.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.hints
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.as_str())
    }

    /// Iterate over the captured hints.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &str)> {
        self.hints
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
    }

    /// A verdict from hints alone, but only when they definitively rule
    /// graphics out.
    ///
    /// A positive hint is deliberately not enough to select the backend: hints
    /// are spoofable and are routinely wrong through SSH, tmux, and nested
    /// sessions, so only the terminal's own reply proves support.
    pub fn definite_verdict(&self) -> Option<CapabilityVerdict> {
        if let Some(term) = self.get("TERM")
            && (term.is_empty() || term.eq_ignore_ascii_case("dumb"))
        {
            return Some(CapabilityVerdict::Unsupported);
        }
        if let Some(program) = self.get("TERM_PROGRAM")
            && program.eq_ignore_ascii_case("Apple_Terminal")
        {
            return Some(CapabilityVerdict::Unsupported);
        }
        None
    }
}

/// Replies parsed out of raw terminal input.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Replies {
    /// Graphics replies, as the text between the control data and the APC
    /// terminator.
    pub graphics: Vec<String>,
    /// Primary device-attributes reply bodies, including the leading `?`.
    pub device_attributes: Vec<String>,
}

impl Replies {
    /// True when at least one graphics reply reported `OK`.
    pub fn graphics_ok(&self) -> bool {
        self.graphics.iter().any(|reply| reply.contains("OK"))
    }

    /// True when the terminal answered the device-attributes request.
    pub fn saw_device_attributes(&self) -> bool {
        !self.device_attributes.is_empty()
    }
}

/// One classified capability probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityProbe {
    /// Classification of the replies.
    pub verdict: CapabilityVerdict,
    /// The last graphics reply, verbatim.
    pub graphics_reply: Option<String>,
    /// The last device-attributes reply body.
    pub device_attributes: Option<String>,
}

/// The outcome of detection, including whether a probe was actually sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    /// Classification used for selection.
    pub verdict: CapabilityVerdict,
    /// The probe, or `None` when the environment hints already ruled graphics
    /// out and no query was sent.
    pub probe: Option<CapabilityProbe>,
}

/// Something that can send a query to the terminal and collect the reply.
///
/// The seam keeps detection testable without a terminal: the real responder
/// owns raw mode and a bounded read, while tests supply canned reply bytes.
pub trait CapabilityResponder {
    /// Send `query` and return the reply bytes received before `timeout`, or
    /// when the reply is complete.
    fn exchange(&mut self, query: &[u8], timeout: Duration) -> io::Result<Vec<u8>>;
}

/// Classify raw reply bytes.
pub fn classify(replies: &Replies) -> CapabilityProbe {
    let graphics_reply = replies.graphics.last().cloned();
    let device_attributes = replies.device_attributes.last().cloned();
    let verdict = if replies.graphics_ok() {
        CapabilityVerdict::Supported
    } else if graphics_reply.is_some() {
        CapabilityVerdict::GraphicsError
    } else if replies.saw_device_attributes() {
        CapabilityVerdict::Unsupported
    } else {
        CapabilityVerdict::NoReply
    };
    CapabilityProbe {
        verdict,
        graphics_reply,
        device_attributes,
    }
}

/// Scan raw terminal input for graphics and device-attributes replies.
///
/// This is a scanner, not a terminal parser: it recognizes the two reply shapes
/// the probe waits for and ignores everything else, such as typed keys, mouse
/// reports, and focus events.
pub fn parse_replies(bytes: &[u8]) -> Replies {
    let mut replies = Replies::default();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == 0x1b
            && bytes.get(index + 1) == Some(&b'_')
            && let Some(body) = between(bytes, index + 3, b"\x1b\\")
        {
            replies
                .graphics
                .push(String::from_utf8_lossy(body).into_owned());
            index += 3 + body.len() + 2;
            continue;
        }
        if bytes[index] == 0x1b
            && bytes.get(index + 1) == Some(&b'[')
            && let Some(end) = (index + 2..bytes.len()).find(|&i| (0x40..=0x7e).contains(&bytes[i]))
        {
            let body = String::from_utf8_lossy(&bytes[index + 2..end]).into_owned();
            if body.starts_with('?') && bytes[end] == b'c' {
                replies.device_attributes.push(body);
            }
            index = end + 1;
            continue;
        }
        index += 1;
    }
    replies
}

/// Find `needle` in `haystack` after `start`, returning the enclosed body.
fn between<'a>(haystack: &'a [u8], start: usize, needle: &[u8]) -> Option<&'a [u8]> {
    haystack[start..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| &haystack[start..start + offset])
}

/// Run the terminal query through `responder` and classify the reply.
pub fn probe(
    responder: &mut dyn CapabilityResponder,
    timeout: Duration,
) -> io::Result<CapabilityProbe> {
    let bytes = responder.exchange(PROBE_QUERY, timeout)?;
    Ok(classify(&parse_replies(&bytes)))
}

/// Detect terminal capabilities from hints and, unless they already rule
/// graphics out, the terminal's own reply.
pub fn detect(
    hints: &EnvironmentHints,
    responder: &mut dyn CapabilityResponder,
    timeout: Duration,
) -> io::Result<Detection> {
    if let Some(verdict) = hints.definite_verdict() {
        return Ok(Detection {
            verdict,
            probe: None,
        });
    }
    let probe = probe(responder, timeout)?;
    Ok(Detection {
        verdict: probe.verdict,
        probe: Some(probe),
    })
}

/// A responder that talks to the process's real terminal.
#[cfg(unix)]
#[derive(Debug, Default)]
pub struct TerminalResponder;

#[cfg(unix)]
impl CapabilityResponder for TerminalResponder {
    fn exchange(&mut self, query: &[u8], timeout: Duration) -> io::Result<Vec<u8>> {
        let was_raw = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
        if !was_raw {
            crossterm::terminal::enable_raw_mode()?;
        }
        let result = exchange_raw(query, timeout);
        if !was_raw {
            let _ = crossterm::terminal::disable_raw_mode();
        }
        result
    }
}

/// Write `query` to the terminal and read replies until the device-attributes
/// reply arrives or `timeout` elapses.
#[cfg(unix)]
fn exchange_raw(query: &[u8], timeout: Duration) -> io::Result<Vec<u8>> {
    use std::os::fd::AsRawFd;

    use mio::unix::SourceFd;
    use mio::{Events, Interest, Poll, Token};

    let mut stdout = io::stdout();
    stdout.write_all(query)?;
    stdout.flush()?;

    let stdin = io::stdin();
    let fd = stdin.as_raw_fd();
    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(4);
    poll.registry()
        .register(&mut SourceFd(&fd), Token(0), Interest::READABLE)?;

    let deadline = Instant::now() + timeout;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        poll.poll(&mut events, Some(deadline - now))?;
        if events.is_empty() {
            break;
        }
        let mut lock = stdin.lock();
        match lock.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => bytes.extend_from_slice(&chunk[..read]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
            Err(error) => return Err(error),
        }
        if parse_replies(&bytes).saw_device_attributes() {
            // A graphics reply can race the device-attributes reply that ends
            // the probe; give it a short grace before stopping.
            if poll.poll(&mut events, Some(REPLY_GRACE)).is_ok() && !events.is_empty() {
                let mut lock = stdin.lock();
                if let Ok(read) = lock.read(&mut chunk) {
                    bytes.extend_from_slice(&chunk[..read]);
                }
            }
            break;
        }
    }
    Ok(bytes)
}

/// A responder for platforms without the Unix terminal probe.
#[cfg(not(unix))]
#[derive(Debug, Default)]
pub struct TerminalResponder;

#[cfg(not(unix))]
impl CapabilityResponder for TerminalResponder {
    fn exchange(&mut self, _query: &[u8], _timeout: Duration) -> io::Result<Vec<u8>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A responder that replays fixed bytes and records how often it was used.
    #[derive(Default)]
    struct FakeResponder {
        replies: Vec<u8>,
        exchanges: usize,
        queries: Vec<Vec<u8>>,
    }

    impl FakeResponder {
        fn replying(replies: impl Into<Vec<u8>>) -> Self {
            Self {
                replies: replies.into(),
                ..Self::default()
            }
        }
    }

    impl CapabilityResponder for FakeResponder {
        fn exchange(&mut self, query: &[u8], _timeout: Duration) -> io::Result<Vec<u8>> {
            self.exchanges += 1;
            self.queries.push(query.to_vec());
            Ok(self.replies.clone())
        }
    }

    const SUPPORTED: &[u8] = b"\x1b_Gi=31;OK\x1b\\\x1b[?62;22;52c";
    const DA_ONLY: &[u8] = b"\x1b[?62;22;52c";
    const GRAPHICS_ERROR: &[u8] = b"\x1b_Gi=31;ENOTSUP:unsupported\x1b\\\x1b[?62c";

    fn hints(pairs: &[(&'static str, &str)]) -> EnvironmentHints {
        EnvironmentHints::from_pairs(pairs.iter().map(|(key, value)| (*key, (*value).to_owned())))
    }

    fn detect_with(pairs: &[(&'static str, &str)], responder: &mut FakeResponder) -> Detection {
        detect(&hints(pairs), responder, Duration::from_millis(10))
            .expect("fake responder never fails")
    }

    #[test]
    fn parses_graphics_and_device_attributes_replies() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"hi\x1b_Gi=31;OK\x1b\\");
        bytes.extend_from_slice(b"\x1b[?62;4c");
        bytes.extend_from_slice(b"\x1b[1;2R");
        let replies = parse_replies(&bytes);
        assert!(replies.graphics_ok());
        assert!(replies.saw_device_attributes());
        assert_eq!(replies.graphics, vec!["i=31;OK".to_owned()]);
        assert_eq!(replies.device_attributes, vec!["?62;4".to_owned()]);
    }

    #[test]
    fn classifies_every_probe_shape() {
        assert_eq!(
            classify(&parse_replies(SUPPORTED)).verdict,
            CapabilityVerdict::Supported
        );
        assert_eq!(
            classify(&parse_replies(DA_ONLY)).verdict,
            CapabilityVerdict::Unsupported
        );
        assert_eq!(
            classify(&parse_replies(GRAPHICS_ERROR)).verdict,
            CapabilityVerdict::GraphicsError
        );
        assert_eq!(
            classify(&parse_replies(b"")).verdict,
            CapabilityVerdict::NoReply
        );
    }

    #[test]
    fn a_positive_probe_is_the_only_supported_verdict() {
        assert!(classify(&parse_replies(SUPPORTED)).verdict.is_supported());
        for bytes in [DA_ONLY, GRAPHICS_ERROR, b""] {
            assert!(!classify(&parse_replies(bytes)).verdict.is_supported());
        }
    }

    #[test]
    fn detection_sends_the_query_and_classifies_the_reply() {
        let mut responder = FakeResponder::replying(SUPPORTED);
        let detection = detect_with(&[("TERM", "xterm-256color")], &mut responder);
        assert_eq!(detection.verdict, CapabilityVerdict::Supported);
        assert_eq!(responder.exchanges, 1);
        // The query must be the graphics probe followed by device attributes.
        assert_eq!(responder.queries[0], PROBE_QUERY);
    }

    #[test]
    fn ambiguous_and_empty_replies_are_unsupported_for_selection() {
        for (bytes, verdict) in [
            (DA_ONLY, CapabilityVerdict::Unsupported),
            (GRAPHICS_ERROR, CapabilityVerdict::GraphicsError),
            (b"".as_slice(), CapabilityVerdict::NoReply),
        ] {
            let mut responder = FakeResponder::replying(bytes);
            let detection = detect_with(&[("TERM", "xterm-256color")], &mut responder);
            assert_eq!(detection.verdict, verdict);
            assert!(!detection.verdict.is_supported());
            assert_eq!(
                BackendKind::resolve(BackendRequest::Auto, detection.verdict),
                BackendKind::Ascii,
                "verdict {verdict:?} must not auto-select Kitty"
            );
        }
    }

    #[test]
    fn a_definite_environment_hint_skips_the_probe() {
        let mut responder = FakeResponder::replying(SUPPORTED);
        let detection = detect_with(&[("TERM", "dumb")], &mut responder);
        assert_eq!(detection.verdict, CapabilityVerdict::Unsupported);
        assert!(detection.probe.is_none());
        assert_eq!(responder.exchanges, 0, "no query should reach the terminal");
    }

    #[test]
    fn a_positive_environment_hint_alone_does_not_select_kitty() {
        // KITTY_WINDOW_ID is spoofable and wrong through nested sessions, so it
        // is a hint, not proof; only a supported probe selects the backend.
        let mut responder = FakeResponder::replying(DA_ONLY);
        let detection = detect_with(
            &[("TERM", "xterm-kitty"), ("KITTY_WINDOW_ID", "1")],
            &mut responder,
        );
        assert_eq!(detection.verdict, CapabilityVerdict::Unsupported);
        assert_eq!(
            BackendKind::resolve(BackendRequest::Auto, detection.verdict),
            BackendKind::Ascii
        );
    }

    #[test]
    fn explicit_requests_override_detection() {
        assert_eq!(
            BackendKind::resolve(BackendRequest::Ascii, CapabilityVerdict::Supported),
            BackendKind::Ascii
        );
        for verdict in [
            CapabilityVerdict::Supported,
            CapabilityVerdict::Unsupported,
            CapabilityVerdict::GraphicsError,
            CapabilityVerdict::NoReply,
        ] {
            assert_eq!(
                BackendKind::resolve(BackendRequest::Kitty, verdict),
                BackendKind::Kitty
            );
        }
    }

    #[test]
    fn auto_selects_kitty_only_when_supported() {
        assert_eq!(
            BackendKind::resolve(BackendRequest::Auto, CapabilityVerdict::Supported),
            BackendKind::Kitty
        );
        for verdict in [
            CapabilityVerdict::Unsupported,
            CapabilityVerdict::GraphicsError,
            CapabilityVerdict::NoReply,
        ] {
            assert_eq!(
                BackendKind::resolve(BackendRequest::Auto, verdict),
                BackendKind::Ascii
            );
        }
    }

    #[test]
    fn backend_flags_round_trip() {
        for request in [
            BackendRequest::Ascii,
            BackendRequest::Kitty,
            BackendRequest::Auto,
        ] {
            assert_eq!(BackendRequest::from_flag(request.as_str()), Some(request));
        }
        assert_eq!(BackendRequest::from_flag("pixels"), None);
    }

    #[test]
    fn environment_hints_report_definite_negatives_only() {
        assert_eq!(
            hints(&[("TERM", "dumb")]).definite_verdict(),
            Some(CapabilityVerdict::Unsupported)
        );
        assert_eq!(
            hints(&[("TERM_PROGRAM", "Apple_Terminal")]).definite_verdict(),
            Some(CapabilityVerdict::Unsupported)
        );
        // A positive or merely unknown hint is not a verdict.
        assert_eq!(hints(&[("TERM", "xterm-kitty")]).definite_verdict(), None);
        assert_eq!(hints(&[("KITTY_WINDOW_ID", "1")]).definite_verdict(), None);
        assert_eq!(hints(&[]).definite_verdict(), None);
    }
}
