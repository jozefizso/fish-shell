use super::fixture::{Fixture, exact, quote};
use super::native::Event;
fn doubled_chunk(chunk: &str) -> String {
    format!(
        "set -l chunk {}; for n in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16; set chunk \"$chunk$chunk\"; end;",
        quote(chunk)
    )
}

pub fn exercise(f: &Fixture) {
    let probe = f.probe();
    let ack = Event::new("stream-receipt");
    // The consumer checks the producer's retained process handle is nonsignalled before
    // acknowledging the entire binary payload. A spool-until-exit scheduler deadlocks.
    let chunk_ready = Event::auto("slow-consumer-ready");
    let next_chunk = Event::auto("slow-consumer-next");
    let command = format!(
        "{probe} n2-produce {} | {probe} n2-consume {} {} {}; printf '%s\\n' $pipestatus",
        quote(&ack.name),
        quote(&ack.name),
        quote(&chunk_ready.name),
        quote(&next_chunk.name)
    );
    let output = std::thread::scope(|scope| {
        let child = scope.spawn(|| f.eval(&command));
        // Every chunk requires a harness roundtrip: this is an intentionally
        // paced consumer with native synchronization, not a timing-based sleep.
        for _ in 0..512 {
            chunk_ready.wait();
            next_chunk.signal();
        }
        child.join().unwrap()
    });
    let stderr = vec![0xa7; 2 * 1024 * 1024];
    exact(
        output,
        0,
        b"binary=33554432;producer-alive;EOF\n0\n0\n",
        &stderr,
    );

    // Internal producers must run after external readers are runnable, not block the
    // evaluator on a full pipe. Internal/internal intentionally uses fish's buffer.
    let text = doubled_chunk("abcdefghijklmnopqrstuvwxyz012345");
    exact(
        f.eval(&format!(
            "{text} printf '%s\\n' \"$chunk\" | {probe} n2-text-consume; printf '%s\\n' $pipestatus"
        )),
        0,
        b"text=2097152;EOF\n0\n0\n",
        b"",
    );
    let mut basenames = b"abcdefghijklmnopqrstuvwx\n".repeat(65536);
    basenames.extend_from_slice(b"0\n0\n");
    exact(
        f.eval(&format!(
            "{probe} n2-lines-produce | path basename; printf '%s\\n' $pipestatus"
        )),
        0,
        &basenames,
        b"",
    );
    let lines = doubled_chunk("folder/abcdefghijklmnopqrstuvwx\n");
    exact(
        f.eval(&format!(
            "{lines} printf '%s' \"$chunk\" | path basename; printf '%s\\n' $pipestatus"
        )),
        0,
        &basenames,
        b"",
    );
    let mut relayed_basenames = b"abcdefghijklmnopqrstuvwx\n".repeat(65536);
    relayed_basenames.extend_from_slice(b"0\n0\n0\n");
    exact(
        f.eval(&format!(
            "{lines} printf '%s' \"$chunk\" | {probe} n2-copy | path basename; printf '%s\\n' $pipestatus"
        )),
        0,
        &relayed_basenames,
        b"",
    );

    exact(
        f.eval(&format!(
            "{probe} n2-broken-produce | {probe} n2-early-consume; printf '%s\\n' $pipestatus"
        )),
        0,
        b"41\n23\n",
        b"",
    );
    exact(
        f.input(
            "path basename -q <&0; path basename <&0",
            b"firstdir/first\notherdir/second\n",
        ),
        0,
        b"second\n",
        b"",
    );
    // Raw binary delivery, including invalid UTF-8 and NUL, is tested independently
    // of text builtins and of the streaming helper's record header.
    exact(
        f.eval(&format!("{probe} n2-binary | {probe} n2-copy")),
        0,
        b"\0\xff\x80\r\nA\0B",
        b"",
    );
    exact(
        f.eval(&format!(
            "printf '' | {probe} n2-copy; printf '%s\\n' $pipestatus"
        )),
        0,
        b"0\n0\n",
        b"",
    );
    // The bound is enforced against the actual fish process, not the helper or a
    // test-local model. The probe samples the retained child handle's peak memory.
    let mut command = f.command(&[]);
    command.arg("-N").arg("-c").arg(format!(
        "{probe} n2-memory-stream {}",
        quote(f.fish.to_str().unwrap())
    ));
    exact(
        super::fixture::run(&mut command, b""),
        0,
        b"bounded-stream=268435456;EOF\n",
        b"",
    );
}
