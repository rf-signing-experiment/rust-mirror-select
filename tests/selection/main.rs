#![cfg(any(feature = "ping-select", feature = "speedtest-select"))]

mod mock;
#[cfg(feature = "ping-select")]
mod ping;
#[cfg(feature = "speedtest-select")]
mod speedtest;
