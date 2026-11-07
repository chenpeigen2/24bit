//! Drive the backend against the live site and print every event with timing.

use std::time::Instant;

use twentyfourbit_gui::backend::{Backend, Request};

fn main() {
    let client = twentyfourbit_core::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .unwrap();
    let cache = twentyfourbit_core::Cache::default_cache();
    let backend = Backend::start(client, cache);

    let start = Instant::now();
    backend.send(Request::Charts);
    backend.send(Request::WhoAmI);

    let deadline = Instant::now() + std::time::Duration::from_secs(45);
    while Instant::now() < deadline {
        for event in backend.poll() {
            println!("{:>6.2}s  {:?}", start.elapsed().as_secs_f64(), brief(&event));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn brief(e: &twentyfourbit_gui::backend::Event) -> String {
    use twentyfourbit_gui::backend::Event::*;
    match e {
        Busy(w) => format!("Busy({w})"),
        Charts(c) => format!("Charts({} songs)", c.songs.len()),
        Account(a) => format!("Account({a:?})"),
        Error(e) => format!("Error({e})"),
        other => format!("{other:?}"),
    }
}
