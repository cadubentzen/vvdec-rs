use vvdec::*;

macro_rules! assert_matches {
    ($a:expr, $b:pat) => {
        assert!(matches!($a, $b));
    };
}

#[test]
fn basic() {
    const DATA: &[u8] = include_bytes!("../tests/short.vvc");

    let mut decoder = Decoder::new().unwrap();
    assert_matches!(
        decoder.decode(DATA, Some(0), Some(0), false),
        Err(Error::TryAgain)
    );

    assert_matches!(decoder.flush(), Ok(Frame {}));
    assert_matches!(decoder.flush(), Ok(Frame {}));
    assert_matches!(decoder.flush(), Ok(Frame {}));

    assert_matches!(decoder.flush(), Err(Error::Eof));
    assert_matches!(decoder.flush(), Err(Error::RestartRequired));
}
