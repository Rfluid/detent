//! Blocking, serialized request/response over one [`Transport`].
//!
//! Used directly by `detentctl` (offline commands, no daemon) and by the daemon's
//! control plane during attach/resync. One request is in flight at a time: HID++
//! firmware processes requests sequentially and answers with the same feature index,
//! function and software ID.
//!
//! Matching rules (docs/specs/hidpp.md, "Requests"):
//! - response: same device index, feature index, function and our swid
//! - error:    feature index 0xFF (2.0) / 0x8F (1.0), original feature/function/swid match
//! - anything else is either a notification (swid 0, queued for the caller) or foreign
//!   traffic (kernel driver, Solaar...) and is skipped

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use detent_hidpp_features::{FeatureTable, RequestError, Requester, feature_set};
use detent_hidpp_proto::{DeviceIndex, FeatureId, Frame, Kind, ReportId, SoftwareId, classify};
use detent_hidpp_transport::Transport;

/// Default per-attempt response timeout. Bluetooth round trips are a few ms to tens of
/// ms; a sleeping device may take longer to wake. Matches the order of magnitude used
/// by prior art (see docs/research/prior-art.md, "Requests").
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(1000);
/// Extra attempts after a timeout (not after a device error).
pub const DEFAULT_RETRIES: u8 = 2;
/// Notifications kept while waiting for responses; older ones are dropped.
const MAX_QUEUED_NOTIFICATIONS: usize = 64;

const ERR_BUSY: u8 = 0x08;

/// HID++ 2.0 error codes (feature index 0xFF reports).
pub fn error_name(code: u8) -> &'static str {
    match code {
        0x00 => "no error",
        0x01 => "unknown",
        0x02 => "invalid argument",
        0x03 => "out of range",
        0x04 => "hardware error",
        0x05 => "logitech internal",
        0x06 => "invalid feature index",
        0x07 => "invalid function",
        0x08 => "busy",
        0x09 => "unsupported",
        _ => "unrecognized",
    }
}

/// Send, retrying a USB stall. A Bolt receiver stalls a write now and then (EPIPE) and
/// takes it again within ~10 ms (measured on the owner's receiver, 2026-11-25;
/// docs/specs/hidpp.md "Receivers"). Control-plane only: it may sleep up to 60 ms.
pub fn send_retrying<T: Transport>(t: &mut T, f: &Frame) -> std::io::Result<()> {
    for wait_ms in [10, 20, 30] {
        match t.send(f) {
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                std::thread::sleep(Duration::from_millis(wait_ms));
            }
            r => return r,
        }
    }
    t.send(f)
}

pub struct DirectRequester<T: Transport> {
    transport: T,
    device: DeviceIndex,
    swid: SoftwareId,
    timeout: Duration,
    retries: u8,
    features: FeatureTable,
    notifications: VecDeque<Frame>,
}

impl<T: Transport> DirectRequester<T> {
    pub fn new(transport: T, device: DeviceIndex) -> Self {
        Self {
            transport,
            device,
            swid: SoftwareId::DETENT,
            timeout: DEFAULT_TIMEOUT,
            retries: DEFAULT_RETRIES,
            features: FeatureTable::default(),
            notifications: VecDeque::new(),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration, retries: u8) -> Self {
        self.timeout = timeout;
        self.retries = retries;
        self
    }

    /// Enumerate the feature table (FeatureSet) and cache it for [`Requester::feature_index`].
    pub fn load_features(&mut self) -> Result<&FeatureTable, RequestError> {
        let table = feature_set::enumerate(self)?;
        self.features = table;
        Ok(&self.features)
    }

    pub fn features(&self) -> &FeatureTable {
        &self.features
    }

    /// Notifications that arrived while we waited for responses (oldest first).
    pub fn drain_notifications(&mut self) -> impl Iterator<Item = Frame> + '_ {
        self.notifications.drain(..)
    }

    /// Wait for the next notification for this device, up to `timeout`.
    pub fn next_notification(&mut self, timeout: Duration) -> Result<Option<Frame>, RequestError> {
        if let Some(f) = self.notifications.pop_front() {
            return Ok(Some(f));
        }
        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            let Some(frame) = self.transport.recv(Some(deadline - now))? else {
                return Ok(None);
            };
            if self.is_ours(&frame) && classify(&frame, self.swid) == Kind::Notification {
                return Ok(Some(frame));
            }
        }
    }

    /// Hand the transport over (e.g. to the data plane after discovery).
    pub fn into_transport(self) -> T {
        self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn device(&self) -> DeviceIndex {
        self.device
    }

    /// Direct (Bluetooth/USB) devices sometimes answer with device index 0x00 instead of
    /// 0xFF (docs/research/prior-art.md, "Transport").
    fn is_ours(&self, frame: &Frame) -> bool {
        let d = frame.device_index();
        d == self.device || (self.device == DeviceIndex::DIRECT && d.0 == 0x00)
    }

    fn queue_notification(&mut self, frame: Frame) {
        if self.notifications.len() >= MAX_QUEUED_NOTIFICATIONS {
            self.notifications.pop_front();
        }
        self.notifications.push_back(frame);
    }

    fn attempt(
        &mut self,
        req: &Frame,
        feature_index: u8,
        function: u8,
    ) -> Result<Option<Frame>, RequestError> {
        send_retrying(&mut self.transport, req)?;
        let deadline = Instant::now() + self.timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            let Some(frame) = self.transport.recv(Some(deadline - now))? else {
                return Ok(None);
            };
            if !self.is_ours(&frame) {
                continue;
            }
            match classify(&frame, self.swid) {
                Kind::Response
                    if frame.feature_index() == feature_index && frame.function() == function =>
                {
                    return Ok(Some(frame));
                }
                Kind::Error {
                    feature_index: fi,
                    function: f,
                    code,
                } if fi == feature_index && f == function => {
                    return Err(RequestError::Device(code));
                }
                Kind::Notification => self.queue_notification(frame),
                _ => {}
            }
        }
    }
}

impl<T: Transport> Requester for DirectRequester<T> {
    fn feature_index(&self, id: FeatureId) -> Option<u8> {
        if id == FeatureId::ROOT {
            return Some(0);
        }
        self.features.index_of(id)
    }

    fn request(
        &mut self,
        feature_index: u8,
        function: u8,
        params: &[u8],
    ) -> Result<Frame, RequestError> {
        // Long reports: every HID++ 2.0 device accepts them, and some Bluetooth devices
        // don't accept short ones (docs/research/prior-art.md, "Transport").
        let rid = if params.len() > ReportId::Long.max_params() {
            ReportId::VeryLong
        } else {
            ReportId::Long
        };
        let req = Frame::request_with(rid, self.device, feature_index, function, self.swid, params)
            .map_err(|_| RequestError::Malformed)?;
        // Retry on timeout and on BUSY (0x08), the only device error the kernel retries.
        let mut last = RequestError::Timeout;
        for attempt in 0..=self.retries {
            match self.attempt(&req, feature_index, function) {
                Ok(Some(resp)) => return Ok(resp),
                Ok(None) => last = RequestError::Timeout,
                Err(RequestError::Device(ERR_BUSY)) => {
                    last = RequestError::Device(ERR_BUSY);
                    std::thread::sleep(Duration::from_millis(20 * (u64::from(attempt) + 1)));
                }
                Err(e) => return Err(e),
            }
        }
        Err(last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use detent_hidpp_transport::mock::MockTransport;

    fn long(bytes: &[u8]) -> Frame {
        let mut b = [0u8; 20];
        b[..bytes.len()].copy_from_slice(bytes);
        Frame::parse(&b).unwrap()
    }

    #[test]
    fn matches_response_and_queues_notifications() {
        let mut t = MockTransport::new();
        // kernel traffic, a notification, then our answer
        t.push_inbound(long(&[0x11, 0xFF, 0x00, 0x11, 1, 2]));
        t.push_inbound(long(&[0x11, 0xFF, 0x05, 0x00, 0, 0xC3]));
        t.push_inbound(long(&[0x11, 0xFF, 0x00, 0x1E, 4, 5, 0x5A]));
        let mut r = DirectRequester::new(t, DeviceIndex::DIRECT);
        let resp = r.request(0, 1, &[0, 0, 0x5A]).unwrap();
        assert_eq!(&resp.params()[..3], &[4, 5, 0x5A]);
        assert_eq!(r.drain_notifications().count(), 1);
        let sent = &r.transport_mut().sent;
        assert_eq!(sent.len(), 1);
        assert_eq!(
            &sent[0].as_bytes()[..7],
            &[0x11, 0xFF, 0x00, 0x1E, 0, 0, 0x5A]
        );
    }

    #[test]
    fn maps_device_errors() {
        let mut t = MockTransport::new();
        // error for a different function first (ignored), then ours
        t.push_inbound(long(&[0x11, 0xFF, 0xFF, 0x05, 0x2E, 0x02]));
        t.push_inbound(long(&[0x11, 0xFF, 0xFF, 0x05, 0x1E, 0x07]));
        let mut r = DirectRequester::new(t, DeviceIndex::DIRECT);
        match r.request(5, 1, &[]) {
            Err(RequestError::Device(0x07)) => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn talks_to_a_receiver_slot() {
        let mut t = MockTransport::new();
        // another slot's notification, a receiver error for an empty slot (not ours),
        // then slot 2's answer: protocol 4.5 (verbatim, owner's Bolt receiver)
        t.push_inbound(long(&[0x11, 0x01, 0x04, 0x00, 1, 1, 1]));
        t.push_inbound(Frame::parse(&[0x10, 0x03, 0x8F, 0x00, 0x1E, 0x09, 0x00]).unwrap());
        t.push_inbound(long(&[0x11, 0x02, 0x00, 0x1E, 0x04, 0x05, 0x5A]));
        let mut r = DirectRequester::new(t, DeviceIndex(2));
        let resp = r.request(0, 1, &[0, 0, 0x5A]).unwrap();
        assert_eq!(&resp.params()[..2], &[4, 5]);
        assert_eq!(r.drain_notifications().count(), 0);
        assert_eq!(r.transport_mut().sent[0].as_bytes()[1], 0x02);
    }

    #[test]
    fn empty_receiver_slot_is_a_device_error() {
        let mut t = MockTransport::new();
        // verbatim: the Bolt receiver's answer to a ping at an empty slot
        t.push_inbound(Frame::parse(&[0x10, 0x01, 0x8F, 0x00, 0x1E, 0x09, 0x00]).unwrap());
        let mut r = DirectRequester::new(t, DeviceIndex(1));
        assert!(matches!(
            r.request(0, 1, &[0, 0, 0x5A]),
            Err(RequestError::Device(0x09))
        ));
    }

    #[test]
    fn times_out_after_retries() {
        let t = MockTransport::new();
        let mut r =
            DirectRequester::new(t, DeviceIndex::DIRECT).with_timeout(Duration::from_millis(1), 2);
        assert!(matches!(r.request(0, 1, &[]), Err(RequestError::Timeout)));
        assert_eq!(r.transport_mut().sent.len(), 3);
    }
}
