use vvdec::*;

macro_rules! assert_matches {
    ($a:expr, $b:pat) => {
        assert!(matches!($a, $b));
    };
}

const DATA: &[u8] = include_bytes!("../tests/short.vvc");

#[test]
fn basic() -> Result<(), Error> {
    let mut params = Params::new();
    params.set_remove_padding(true);

    let mut decoder = Decoder::with_params(params).unwrap();
    assert_matches!(
        decoder.decode(DATA, Some(0), Some(0), false),
        Err(Error::TryAgain)
    );

    let frame1 = decoder.flush()?;
    println!("{frame1}");
    let plane = frame1.plane(PlaneComponent::Y);
    println!("plane 0: {} len {}", plane, plane.len());
    let plane = frame1.plane(PlaneComponent::U);
    println!("plane 1: {} len {}", plane, plane.len());
    let plane = frame1.plane(PlaneComponent::V);
    println!("plane 2: {} len {}", plane, plane.len());

    let frame2 = decoder.flush()?;
    println!("{frame2}");

    let frame3 = decoder.flush()?;
    println!("{frame3}");

    assert_matches!(decoder.flush(), Err(Error::Eof));
    assert_matches!(decoder.flush(), Err(Error::RestartRequired));

    Ok(())
}

#[test]
fn split_data() -> Result<(), Error> {
    let mut decoder = Decoder::new().unwrap();

    const ANNEX_B_START_CODE: &[u8] = &[0, 0, 0, 1];
    let mut indices: Vec<_> = DATA
        .windows(4)
        .enumerate()
        .filter(|(_, window)| *window == ANNEX_B_START_CODE)
        .map(|(i, _)| i)
        .collect();
    indices.push(DATA.len());
    for pair in indices.windows(2) {
        let sub_slice = &DATA[pair[0]..pair[1]];
        let _ = decoder.decode(sub_slice, Some(0), Some(0), false);
    }

    let frame1 = decoder.flush()?;
    println!("{frame1}");
    let plane = frame1.plane(PlaneComponent::Y);
    println!("plane 0: {} len {}", plane, plane.len());
    let plane = frame1.plane(PlaneComponent::U);
    println!("plane 1: {} len {}", plane, plane.len());
    let plane = frame1.plane(PlaneComponent::V);
    println!("plane 2: {} len {}", plane, plane.len());

    let frame2 = decoder.flush()?;
    println!("{frame2}");

    let frame3 = decoder.flush()?;
    println!("{frame3}");

    assert_matches!(decoder.flush(), Err(Error::Eof));
    assert_matches!(decoder.flush(), Err(Error::RestartRequired));

    Ok(())
}
