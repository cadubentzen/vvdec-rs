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

    let frame1 = decoder.flush().unwrap();
    println!("{frame1}");
    let plane = frame1.plane(0).unwrap();
    println!("plane 0: {} len {}", plane, plane.len());
    let plane = frame1.plane(1).unwrap();
    println!("plane 1: {} len {}", plane, plane.len());
    let plane = frame1.plane(2).unwrap();
    println!("plane 2: {} len {}", plane, plane.len());

    let frame2 = decoder.flush().unwrap();
    println!("{frame2}");

    let frame3 = decoder.flush().unwrap();
    println!("{frame3}");

    assert_matches!(decoder.flush(), Err(Error::Eof));
    assert_matches!(decoder.flush(), Err(Error::RestartRequired));
}
