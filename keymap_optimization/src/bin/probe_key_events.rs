// Probe tool: prints every key press and release from one Linux input device, with kernel timestamps.
// With no argument, lists the available devices. Needs read access to /dev/input (root or the `input` group).
// The device is grabbed while this runs, so its keystrokes go nowhere else; stop with Ctrl-C from another keyboard.

use evdev::{Device, EventSummary};

// evdev key event values
const RELEASE: i32 = 0;
const PRESS: i32 = 1;
const AUTOREPEAT: i32 = 2;

fn list_devices() {
    for (path, device) in evdev::enumerate() {
        println!("{}\t{}", path.display(), device.name().unwrap_or("<unnamed>"));
    }
}

fn print_key_events(device_path: &str) {
    let mut device = Device::open(device_path).expect("failed to open device");
    device.grab().expect("failed to grab device");
    println!("ms since first event\tevent\tkey");

    let mut first_timestamp = None;
    loop {
        for event in device.fetch_events().expect("failed to read events") {
            let timestamp = event.timestamp();
            if let EventSummary::Key(_, key, value) = event.destructure() {
                let since_first = timestamp.duration_since(*first_timestamp.get_or_insert(timestamp))
                                           .expect("event timestamps went backwards");
                let kind = match value {
                    RELEASE => "release",
                    PRESS => "press",
                    AUTOREPEAT => "repeat",
                    other => panic!("unknown key event value {}", other),
                };
                println!("{:.1}\t{}\t{:?}", since_first.as_secs_f64() * 1000.0, kind, key);
            }
        }
    }
}

fn main() {
    match std::env::args().nth(1) {
        None => list_devices(),
        Some(device_path) => print_key_events(&device_path),
    }
}
