//! `Requester` over the data plane's ring: the worker's only way to talk to the device,
//! so the data plane stays the only reader/writer of the hidraw fd.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use detent_device::requester::{DEFAULT_RETRIES, DEFAULT_TIMEOUT};
use detent_hidpp_features::{FeatureTable, RequestError, Requester};
use detent_hidpp_proto::{FeatureId, Frame, Kind, ReportId, SoftwareId, classify};

use super::Link;
use crate::msg::{FromDp, ToDp, WorkerMsg};

/// Messages that arrive while waiting for a response are kept in `backlog` for the
/// worker loop.
pub(super) struct RingRequester<'a> {
    pub(super) link: &'a mut Link,
    pub(super) rx: &'a Receiver<WorkerMsg>,
    pub(super) backlog: &'a mut VecDeque<WorkerMsg>,
    pub(super) features: &'a FeatureTable,
    pub(super) gone: &'a mut Option<&'static str>,
}

fn not_connected(why: &'static str) -> RequestError {
    RequestError::Io(std::io::Error::new(std::io::ErrorKind::NotConnected, why))
}

impl RingRequester<'_> {
    /// A long request to this link's device index (0xFF direct, 1–6 behind a receiver).
    fn frame(&self, fi: u8, func: u8, params: &[u8]) -> Result<Frame, RequestError> {
        Frame::request_with(
            ReportId::Long,
            self.link.device,
            fi,
            func,
            SoftwareId::DETENT,
            params,
        )
        .map_err(|_| RequestError::Malformed)
    }

    fn attempt(&mut self, req: &Frame, fi: u8, func: u8) -> Result<Option<Frame>, RequestError> {
        self.link.send(ToDp::Send(*req));
        let deadline = Instant::now() + DEFAULT_TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(None);
            }
            match self.rx.recv_timeout(left) {
                Ok(WorkerMsg::Dp(FromDp::Response(f))) => match classify(&f, SoftwareId::DETENT) {
                    Kind::Response if f.feature_index() == fi && f.function() == func => {
                        return Ok(Some(f));
                    }
                    Kind::Error {
                        feature_index,
                        function,
                        code,
                    } if feature_index == fi && function == func => {
                        return Err(RequestError::Device(code));
                    }
                    _ => {} // stale answer to an earlier, timed-out request
                },
                Ok(WorkerMsg::Dp(FromDp::Gone(r))) => {
                    *self.gone = Some(r);
                    return Err(not_connected(r));
                }
                Ok(other) => self.backlog.push_back(other),
                Err(RecvTimeoutError::Timeout) => return Ok(None),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(not_connected("data plane gone"));
                }
            }
        }
    }
}

impl Requester for RingRequester<'_> {
    fn feature_index(&self, id: FeatureId) -> Option<u8> {
        if id == FeatureId::ROOT {
            return Some(0);
        }
        self.features.index_of(id)
    }

    fn request(&mut self, fi: u8, func: u8, params: &[u8]) -> Result<Frame, RequestError> {
        if self.gone.is_some() {
            return Err(not_connected("device gone"));
        }
        let req = self.frame(fi, func, params)?;
        let mut last = RequestError::Timeout;
        for attempt in 0..=DEFAULT_RETRIES {
            match self.attempt(&req, fi, func) {
                Ok(Some(f)) => return Ok(f),
                Ok(None) => last = RequestError::Timeout,
                Err(RequestError::Device(0x08)) => {
                    last = RequestError::Device(0x08);
                    std::thread::sleep(Duration::from_millis(20 * (u64::from(attempt) + 1)));
                }
                Err(e) => return Err(e),
            }
        }
        Err(last)
    }

    fn send(&mut self, fi: u8, func: u8, params: &[u8]) -> Result<(), RequestError> {
        if self.gone.is_some() {
            return Err(not_connected("device gone"));
        }
        let req = self.frame(fi, func, params)?;
        self.link.send(ToDp::Send(req));
        Ok(())
    }
}
