//! Public diagnostics for the syntax parser shared with the operator gate.
use fa_reference::strict_json::{ErrorKind, Limits, parse};

#[test]
fn invalid_utf8_positions_use_the_valid_prefix_and_oversize_coordinates_are_unavailable() {
    let valid = " \n\"éx\"".as_bytes();
    assert!(parse(valid, Limits::default()).is_ok());
    let mut invalid = valid.to_vec();
    invalid[5] = 0xff;
    let error = parse(&invalid, Limits::default()).unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidUtf8);
    assert_eq!((error.offset, error.line, error.column), (5, 2, 3));

    let at_cap = Limits {
        max_bytes: valid.len(),
        ..Limits::default()
    };
    assert!(parse(valid, at_cap).is_ok());
    let error = parse(
        valid,
        Limits {
            max_bytes: 2,
            ..at_cap
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::SizeLimit);
    assert_eq!((error.offset, error.line, error.column), (2, 0, 0));
    // The byte limit can bisect a UTF-8 character. No character position is
    // fabricated and no UTF-8 validation of the oversized document is needed.
    let error = parse(
        "\"é\"".as_bytes(),
        Limits {
            max_bytes: 2,
            ..at_cap
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::SizeLimit);
    assert_eq!((error.offset, error.line, error.column), (2, 0, 0));
}
