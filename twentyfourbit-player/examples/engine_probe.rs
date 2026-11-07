//! Watch the engine's position advance against wall clock.

use std::time::{Duration, Instant};

fn main() {
    let path = std::env::args().nth(1).expect("usage: engine_probe <file>");
    let e = twentyfourbit_player::Engine::start().unwrap();
    e.play(std::path::PathBuf::from(&path));
    let start = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(200));
        let s = e.snapshot();
        match &s.transport {
            Some(t) => println!(
                "{:>6.2}s wall  pos={:>6.2}s phase={:?} src={} out={} resample={} buffering={}",
                start.elapsed().as_secs_f64(),
                t.position().as_secs_f64(),
                t.phase(),
                s.source_rate,
                s.output_rate,
                s.resampling,
                s.buffering
            ),
            None => println!("{:>6.2}s wall  (no transport)", start.elapsed().as_secs_f64()),
        }
        if start.elapsed() > Duration::from_secs(4) {
            break;
        }
    }
    e.shutdown();
}
