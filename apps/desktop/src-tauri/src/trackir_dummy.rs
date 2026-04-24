#![cfg_attr(windows, windows_subsystem = "windows")]

use std::{thread, time::Duration};

fn main() {
 loop {
  thread::sleep(Duration::from_secs(60));
 }
}
