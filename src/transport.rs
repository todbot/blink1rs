// SPDX-FileCopyrightText: 2026 Tod Kurt
// SPDX-License-Identifier: MIT

//! How reports reach the device.
//!
//! Kept private: a public transport trait would be public surface to keep
//! compatible forever, and nothing outside the crate needs it.

use crate::error::Result;
use crate::protocol::Report;

/// `Send` is load-bearing. `hidapi::HidDevice` is `Send + !Sync`, but boxing
/// a trait object drops auto traits unless they are named, and callers want
/// to move a `Blink1` into a worker thread.
pub(crate) trait Transport: Send {
    fn send(&self, buf: &Report) -> Result<()>;

    /// Send the report, then read the device's answer back over it.
    ///
    /// The send is not optional: it is how the LED index or pattern position
    /// argument reaches the device (`blink1_read` in
    /// `blink1-lib-lowlevel-hidapi.h:183`).
    fn recv(&self, buf: &mut Report) -> Result<()>;
}

pub(crate) struct HidTransport {
    dev: hidapi::HidDevice,
}

impl HidTransport {
    pub(crate) fn new(dev: hidapi::HidDevice) -> HidTransport {
        HidTransport { dev }
    }
}

impl Transport for HidTransport {
    fn send(&self, buf: &Report) -> Result<()> {
        self.dev.send_feature_report(buf)?;
        Ok(())
    }

    fn recv(&self, buf: &mut Report) -> Result<()> {
        self.dev.send_feature_report(buf)?;
        self.dev.get_feature_report(buf)?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod mock {
    use super::*;
    use crate::error::Error;
    use std::sync::Mutex;

    /// Records every report sent and replays queued responses.
    #[derive(Default)]
    pub(crate) struct MockTransport {
        inner: Mutex<Inner>,
    }

    #[derive(Default)]
    struct Inner {
        sent: Vec<Report>,
        responses: Vec<Report>,
        fail_after: Option<usize>,
    }

    impl MockTransport {
        pub(crate) fn new() -> MockTransport {
            MockTransport::default()
        }

        /// Queue one canned answer for the next `recv`.
        pub(crate) fn push_response(&self, r: Report) -> &Self {
            self.inner.lock().unwrap().responses.push(r);
            self
        }

        /// Make every call from the `n`th onward return a HID error.
        pub(crate) fn fail_after(&self, n: usize) -> &Self {
            self.inner.lock().unwrap().fail_after = Some(n);
            self
        }

        /// Every report sent so far, in order.
        pub(crate) fn sent(&self) -> Vec<Report> {
            self.inner.lock().unwrap().sent.clone()
        }

        fn record(&self, buf: &Report) -> Result<()> {
            let mut inner = self.inner.lock().unwrap();
            inner.sent.push(*buf);
            match inner.fail_after {
                Some(n) if inner.sent.len() > n => {
                    Err(Error::Hid(hidapi::HidError::HidApiErrorEmpty))
                }
                _ => Ok(()),
            }
        }
    }

    impl Transport for MockTransport {
        fn send(&self, buf: &Report) -> Result<()> {
            self.record(buf)
        }

        fn recv(&self, buf: &mut Report) -> Result<()> {
            self.record(buf)?;
            let mut inner = self.inner.lock().unwrap();
            if inner.responses.is_empty() {
                return Err(Error::BadResponse);
            }
            *buf = inner.responses.remove(0);
            Ok(())
        }
    }
}
