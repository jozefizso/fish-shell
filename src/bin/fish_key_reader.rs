#[cfg(unix)]
use fish::builtins::fish_key_reader;

#[cfg(unix)]
fn main() {
    fish_key_reader::main();
}

#[cfg(windows)]
fn main() {
    eprintln!("fish_key_reader is not supported by the native Windows prototype");
    std::process::exit(2);
}
