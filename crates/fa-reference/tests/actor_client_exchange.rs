use fa_reference::action::consequence::oversight::actor::Knowledge;
use fa_reference::action::consequence::oversight::actor_wire::{Command, WireResponse, encode_command, ResponseError};
use fa_reference::action::consequence::oversight::actor_wire::client::*;
use std::io::{self, Cursor, Read, Write};

struct Duplex { input: Cursor<Vec<u8>>, output: Vec<u8>, chunk: usize, flush_blocks: usize, interruptions: usize, reads: usize }
impl Duplex {
    fn new(input: Vec<u8>) -> Self { Self { input: Cursor::new(input), output: Vec::new(), chunk: usize::MAX, flush_blocks: 0, interruptions: 0, reads: 0 } }
}
impl Read for Duplex {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.reads += 1; let n = bytes.len().min(self.chunk); self.input.read(&mut bytes[..n])
    }
}
impl Write for Duplex {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.interruptions > 0 { self.interruptions -= 1; return Err(io::ErrorKind::Interrupted.into()); }
        let n = bytes.len().min(self.chunk); self.output.extend_from_slice(&bytes[..n]); Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.flush_blocks > 0 { self.flush_blocks -= 1; return Err(io::ErrorKind::WouldBlock.into()); }
        Ok(())
    }
}
fn response(id: u64) -> Vec<u8> {
    let mut bytes = WireResponse { request: Some(id), result: Ok(Knowledge::Pending { request: id }) }.encode();
    bytes.push(b'\n'); bytes
}
fn run(exchange: &mut ActorExchange<Duplex>, budget: &mut ClientIoBudget) -> Result<WireResponse, ClientError> {
    for _ in 0..8192 {
        if let ClientProgress::Response(response) = exchange.step(budget)? { return Ok(response); }
    }
    panic!("bounded exchange did not finish")
}

#[test]
fn partial_io_and_blocked_flush_deliver_exactly_one_original_command_and_response() {
    let mut stream = Duplex::new(response(17)); stream.chunk = 3; stream.flush_blocks = 2; stream.interruptions = 2;
    let command = Command::Poll { request: 17 }; let mut expected = encode_command(&command).unwrap(); expected.push(b'\n');
    let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
    let mut exchange = ActorExchange::new(stream, command, &mut budget).unwrap();
    while exchange.phase() == ClientPhase::Sending {
        exchange.step(&mut budget).unwrap(); assert_eq!(budget.work().read_bytes, 0);
    }
    let observed = run(&mut exchange, &mut budget).unwrap();
    assert_eq!(observed.result, Ok(Knowledge::Pending { request: 17 }));
    let done = budget.work();
    assert_eq!(exchange.step(&mut budget).unwrap(), ClientProgress::Complete);
    assert_eq!(budget.work(), done); assert_eq!(done.exchanges, 1);
    assert_eq!(done.written_bytes, expected.len() as u64); assert_eq!(done.read_bytes, response(17).len() as u64);
    let stream = exchange.into_stream().unwrap(); assert_eq!(stream.output, expected); assert!(stream.reads > 1);
}

#[test]
fn interrupted_and_would_block_calls_cannot_bypass_the_lifetime_budget() {
    let mut stream = Duplex::new(response(1)); stream.interruptions = usize::MAX;
    let mut budget = ClientIoBudget::new(ClientIoLimits { calls: 3, ..ClientIoLimits::default() }).unwrap();
    let mut exchange = ActorExchange::new(stream, Command::Poll { request: 1 }, &mut budget).unwrap();
    for _ in 0..3 { assert_eq!(exchange.step(&mut budget).unwrap(), ClientProgress::Blocked); }
    assert_eq!(exchange.step(&mut budget), Err(ClientError::Limit));
    let failure = exchange.failure().unwrap(); assert!(failure.request_may_have_reached_peer);
    assert_eq!(failure.written_bytes, 0); assert_eq!(budget.work().calls, 3);
    assert_eq!(budget.work().written_bytes, 0); assert!(exchange.into_stream().is_none());
    assert_eq!(ActorExchange::new(Duplex::new(response(1)), Command::Poll { request: 1 }, &mut budget).unwrap_err().error, ClientError::Limit);
}

#[test]
fn eof_wrong_request_oversize_and_extra_frames_never_return_an_observation() {
    let mut truncated = response(7); truncated.pop();
    let mut pipelined = response(7); pipelined.extend_from_slice(&response(7));
    for (bytes, expected) in [
        (truncated, ClientError::Io(io::ErrorKind::UnexpectedEof)),
        (response(8), ClientError::Response(ResponseError::Binding)),
        (vec![b' '; 513], ClientError::Response(ResponseError::Capacity)),
        (pipelined, ClientError::Response(ResponseError::Malformed)),
    ] {
        let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
        let mut exchange = ActorExchange::new(Duplex::new(bytes), Command::Poll { request: 7 }, &mut budget).unwrap();
        assert_eq!(run(&mut exchange, &mut budget), Err(expected));
        assert_eq!(exchange.phase(), ClientPhase::Failed); assert!(exchange.response().is_none());
        let work = budget.work(); assert_eq!(exchange.step(&mut budget), Err(expected)); assert_eq!(budget.work(), work);
        assert!(exchange.failure().unwrap().request_may_have_reached_peer);
        assert!(exchange.into_stream().is_none());
    }
}

#[test]
fn exact_byte_limits_succeed_and_one_less_refuses_without_replenishing_work() {
    let command = Command::Poll { request: 5 };
    let written = encode_command(&command).unwrap().len() as u64 + 1; let read = response(5).len() as u64;
    for allowed in [read - 1, read] {
        let mut budget = ClientIoBudget::new(ClientIoLimits { exchanges: 1, written_bytes: written, read_bytes: allowed,
            ..ClientIoLimits::default() }).unwrap();
        let mut exchange = ActorExchange::new(Duplex::new(response(5)), command.clone(), &mut budget).unwrap();
        let result = run(&mut exchange, &mut budget);
        if allowed == read { assert!(result.is_ok()); } else { assert_eq!(result, Err(ClientError::Limit)); }
        assert_eq!(budget.work().written_bytes, written); assert_eq!(budget.work().read_bytes, allowed);
        assert_eq!(budget.work().exchanges, 1);
    }
    let mut budget = ClientIoBudget::new(ClientIoLimits { written_bytes: written - 1, ..ClientIoLimits::default() }).unwrap();
    let failure = ActorExchange::new(Duplex::new(response(5)), command, &mut budget).unwrap_err();
    assert_eq!(failure.error, ClientError::Limit); assert!(failure.stream.output.is_empty());
    assert_eq!(budget.work(), ClientIoWork::default());
}

#[test]
fn command_validation_and_abandonment_do_not_manufacture_cancellation() {
    let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
    let refused = ActorExchange::new(Duplex::new(Vec::new()), Command::Poll { request: 0 }, &mut budget).unwrap_err();
    assert!(matches!(refused.error, ClientError::Command(_))); assert_eq!(budget.work(), ClientIoWork::default());
    let mut exchange = ActorExchange::new(refused.stream, Command::Cancel { request: 1 }, &mut budget).unwrap();
    exchange.step(&mut budget).unwrap();
    assert_eq!(exchange.phase(), ClientPhase::Receiving); assert!(exchange.response().is_none());
    assert!(exchange.into_stream().is_none()); assert_eq!(budget.work().exchanges, 1);
}

#[test]
fn a_caught_io_unwind_cannot_replay_bytes_or_replenish_its_admitted_call() {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    struct Panicking { writes: Rc<Cell<usize>> }
    impl Read for Panicking { fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { panic!("unexpected read") } }
    impl Write for Panicking {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.writes.set(self.writes.get() + bytes.len()); panic!("simulated write accepted bytes then unwound")
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    let writes = Rc::new(Cell::new(0)); let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
    let mut exchange = ActorExchange::new(Panicking { writes: Rc::clone(&writes) }, Command::Cancel { request: 1 }, &mut budget).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| exchange.step(&mut budget))).is_err());
    let count = writes.get(); assert!(count > 0); let work = budget.work(); assert_eq!(work.calls, 1);
    assert_eq!(exchange.step(&mut budget), Err(ClientError::InterruptedOperation));
    assert_eq!(budget.work(), work); assert_eq!(writes.get(), count);
    assert!(exchange.failure().unwrap().request_may_have_reached_peer); assert!(exchange.into_stream().is_none());
}
