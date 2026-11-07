//! Time `play_file` against a known-duration file.
//! Second arg: print internal plan decisions (requires RUST_LOG=debug).

fn main() {
    let path = std::env::args().nth(1).expect("usage: measureplay <flac>");
    let t = std::time::Instant::now();
    match twentyfourbit_player::play_file(std::path::Path::new(&path)) {
        Ok(()) => println!("play_file returned after {:?}", t.elapsed()),
        Err(e) => println!("error: {e} after {:?}", t.elapsed()),
    }
}
