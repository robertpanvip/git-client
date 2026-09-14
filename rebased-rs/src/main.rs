mod ui;

use std::path::PathBuf;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| ".".to_string());
    ui::run(PathBuf::from(path));
}
