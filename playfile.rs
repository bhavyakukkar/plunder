use std::time::Duration;

fn main() {
    let file1 = std::env::args().nth(1).expect("Expecting file path");
    // let file2 = std::env::args().nth(2).expect("Expecting file path");
    std::thread::spawn(move || plunder::play_audio(&file1));
    // std::thread::spawn(move || plunder::play_audio(&file2));
    std::thread::sleep(Duration::from_secs(7 * 60));
}
