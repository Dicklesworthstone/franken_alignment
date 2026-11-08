//! The reverse projection cannot turn ambiguous or rebound bookkeeping into a
//! Known navigation link. Corruptions here are deliberate in-memory test inputs.
use super::*;

#[test]
fn original_request_projection_refuses_ambiguous_or_rebound_admitted_identities() {
    let root = Directory::new();
    let p = profile();
    let (mut host, _key, _action) = prepared(&root, &p);
    authorize_request(&mut host, 900);
    let book = &mut host.machine.requests;
    assert_eq!(
        book.admitted_request_index().unwrap(),
        BTreeMap::from([(1, 700), (2, 900)])
    );
    let original = book.records[&900].status;

    // These two externally distinct requests may not claim the same native
    // attempt, even if an inconsistent internal allocation repeats that ID.
    let record = book.records.get_mut(&900).unwrap();
    record.allocated_attempt = 1;
    record.status.disposition = FileRequestDisposition::Admitted {
        attempt: 1,
        stage: ActionState::Authorized,
    };
    assert_eq!(book.admitted_request_index(), Err(Error::Binding));

    let record = book.records.get_mut(&900).unwrap();
    record.allocated_attempt = 2;
    record.status = original;
    record.status.request = 700;
    assert_eq!(book.admitted_request_index(), Err(Error::Binding));

    let record = book.records.get_mut(&900).unwrap();
    record.status = original;
    record.allocated_attempt = 99;
    assert_eq!(book.admitted_request_index(), Err(Error::Binding));
    book.records.get_mut(&900).unwrap().allocated_attempt = 2;
    assert_eq!(
        book.admitted_request_index().unwrap(),
        BTreeMap::from([(1, 700), (2, 900)])
    );
}
