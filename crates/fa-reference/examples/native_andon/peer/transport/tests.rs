use super::*;
use std::{cell::Cell, net::Shutdown};

#[test]
fn exact_eof_documents_keep_bytes_and_oversize_or_empty_refuse() {
    for size in [0, 1, 8192, 8193] {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        client.write_all(&vec![b'x'; size]).unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        let result = read(&mut server, 8192, &mut || Ok(()), &mut || panic!("already complete"));
        match size {
            1 | 8192 => assert_eq!(result.unwrap(), vec![b'x'; size]),
            _ => assert!(result.is_err()),
        }
    }
}

#[test]
fn stalled_and_trickling_peers_cannot_renew_the_invocation() {
    for trickle in [false, true] {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        let turns = Cell::new(0);
        let mut checkpoint = || if turns.get() < 4 { Ok(()) } else { Err("expired".into()) };
        let mut wait = || {
            turns.set(turns.get() + 1);
            if trickle { client.write_all(b"x").unwrap(); }
        };
        assert_eq!(read(&mut server, 8192, &mut checkpoint, &mut wait).unwrap_err(), "expired");
        assert_eq!(turns.get(), 4);
        // Same peer/data, now with EOF and a genuine fresh caller-owned turn.
        client.shutdown(Shutdown::Write).unwrap();
        if trickle {
            assert_eq!(read(&mut server, 8192, &mut || Ok(()), &mut || {}).unwrap(), b"x");
        }
    }
}

#[test]
fn progress_and_last_eof_are_checked_before_returning_completion() {
    let (mut client, mut server) = UnixStream::pair().unwrap();
    client.write_all(b"ready").unwrap(); client.shutdown(Shutdown::Write).unwrap();
    let calls = Cell::new(0);
    let error = read(&mut server, 8192, &mut || {
        calls.set(calls.get() + 1);
        if calls.get() < 5 { Ok(()) } else { Err("expired at EOF".into()) }
    }, &mut || panic!("not blocked")).unwrap_err();
    assert_eq!(error, "expired at EOF");
    assert_eq!(calls.get(), 5);
}

#[test]
fn backpressured_write_expires_without_automatic_repeat() {
    let (mut client, mut server) = UnixStream::pair().unwrap();
    client.set_nonblocking(true).unwrap();
    let mut queued = 0;
    loop {
        match client.write(&[b'q'; 4096]) {
            Ok(count) => queued += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            other => panic!("unexpected queue fill: {other:?}"),
        }
        assert!(queued < 16 * 1024 * 1024);
    }
    let waits = Cell::new(0);
    let error = write(&mut client, b"new", &mut || {
        if waits.get() < 2 { Ok(()) } else { Err("expired under backpressure".into()) }
    }, &mut || waits.set(waits.get() + 1)).unwrap_err();
    assert_eq!(error, "expired under backpressure");
    client.shutdown(Shutdown::Write).unwrap();
    let mut bytes = Vec::new(); server.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, vec![b'q'; queued]);
    let (mut client, mut server) = UnixStream::pair().unwrap();
    write(&mut client, b"new", &mut || Ok(()), &mut || {}).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    assert_eq!(read(&mut server, 16, &mut || Ok(()), &mut || {}).unwrap(), b"new");
}

#[test]
fn expired_admission_neither_reads_nor_writes_and_native_output_is_bounded() {
    let (mut client, mut server) = UnixStream::pair().unwrap();
    client.write_all(b"kept").unwrap(); client.shutdown(Shutdown::Write).unwrap();
    assert!(read(&mut server, 16, &mut || Err("expired".into()), &mut || panic!("no wait")).is_err());
    assert_eq!(read(&mut server, 16, &mut || Ok(()), &mut || {}).unwrap(), b"kept");
    let (mut client, mut server) = UnixStream::pair().unwrap();
    assert!(write(&mut client, b"private", &mut || Err("expired".into()), &mut || panic!("no wait")).is_err());
    client.shutdown(Shutdown::Write).unwrap();
    let mut bytes = Vec::new(); server.read_to_end(&mut bytes).unwrap(); assert!(bytes.is_empty());
    let mut output = Output::default(); output.write_all(&vec![0; MAX_MESSAGE_BYTES]).unwrap();
    assert!(output.write_all(b"x").is_err()); assert_eq!(output.0.len(), MAX_MESSAGE_BYTES);
}
