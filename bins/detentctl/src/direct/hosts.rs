//! Easy-Switch straight on the device (daemon not running). Slots are 0-based here.

use anyhow::Result;
use detent_hidpp_features::RequestError;
use detent_hidpp_features::hosts::{self, HostsState, Target};

use super::{open, req_err};
use crate::exit::{self, Exit};

/// Range and capability problems are the user's input (exit 65), the rest is the device.
fn err(e: RequestError) -> anyhow::Error {
    match e {
        RequestError::OutOfRange(m) => Exit::new(exit::DATAERR, m).into(),
        RequestError::Unsupported(_) => {
            Exit::new(exit::NOINPUT, "this device has no Easy-Switch hosts").into()
        }
        other => req_err(other),
    }
}

pub fn read(sel: Option<&str>) -> Result<HostsState> {
    let (_, mut dev) = open(sel)?;
    hosts::read(&mut dev).map_err(err)
}

/// `(from, to)`; equal when the mouse was already there.
pub fn switch(sel: Option<&str>, to: Target) -> Result<(u8, u8)> {
    let (_, mut dev) = open(sel)?;
    let (_, from, _) = hosts::info(&mut dev).map_err(err)?;
    let to = hosts::switch(&mut dev, to).map_err(err)?.unwrap_or(from);
    Ok((from, to))
}

pub fn rename(sel: Option<&str>, slot: u8, name: &str) -> Result<HostsState> {
    let (_, mut dev) = open(sel)?;
    hosts::rename(&mut dev, slot, name).map_err(err)
}
