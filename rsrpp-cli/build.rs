use std::process::Command;
fn main() {
    Command::new("sh").arg("build.sh").status().expect("Failed to install dependencies");
}
