#[cfg(unix)]
use fish::builtins::fish_indent;

#[cfg(unix)]
fn main() {
    fish_indent::main();
}

#[cfg(windows)]
fn main() {
    eprintln!("fish_indent is not supported by the native Windows prototype");
    std::process::exit(2);
}
