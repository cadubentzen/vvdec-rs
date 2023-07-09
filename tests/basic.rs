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

fn split_data(data: &[u8]) -> Vec<&[u8]> {
    const ANNEX_B_START_CODE: &[u8] = &[0, 0, 0, 1];
    let mut indices: Vec<_> = DATA
        .windows(4)
        .enumerate()
        .filter(|(_, window)| *window == ANNEX_B_START_CODE)
        .map(|(i, _)| i)
        .collect();
    indices.push(DATA.len());

    indices
        .windows(2)
        .map(|pair| &data[pair[0]..pair[1]])
        .collect()
}

#[test]
fn test_split_data() -> Result<(), Error> {
    let mut decoder = Decoder::new().unwrap();

    for slice in split_data(DATA) {
        let _ = decoder.decode(slice, Some(0), Some(0), false);
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

#[test]
fn test_decode_after_flush() -> Result<(), Error> {
    let mut decoder = Decoder::new().unwrap();

    let mut slices = split_data(DATA).into_iter();
    let sps = slices.next().unwrap();
    let pps = slices.next().unwrap();
    let frame1 = slices.next().unwrap();
    let frame2 = slices.next().unwrap();
    let frame3 = slices.next().unwrap();

    let _ = decoder.decode(sps, None, None, false);
    let _ = decoder.decode(pps, None, None, false);
    let _ = decoder.decode(frame1, None, None, false);
    let _ = decoder.decode(frame2, None, None, false);

    assert!(decoder.flush().is_ok());
    assert!(decoder.flush().is_ok());
    assert_eq!(decoder.flush().unwrap_err(), Error::Eof);

    let _ = decoder.decode(sps, None, None, false);
    let _ = decoder.decode(pps, None, None, false);
    let _ = decoder.decode(frame1, None, None, false);
    let _ = decoder.decode(frame2, None, None, false);
    let _ = decoder.decode(frame3, None, None, false);
    assert!(decoder.flush().is_ok());
    assert!(decoder.flush().is_ok());
    assert!(decoder.flush().is_ok());
    assert_eq!(decoder.flush().unwrap_err(), Error::Eof);

    Ok(())
}

#[test]
fn test_change_resolution() -> Result<(), Error> {
    let mut decoder = Decoder::new().unwrap();

    let _ = decoder.decode(DATA, None, None, false);
    let first_frame = decoder.flush().unwrap();
    assert_eq!(first_frame.width(), 320);
    assert_eq!(first_frame.height(), 240);

    const SECOND_DATA: &[u8] = include_bytes!("../tests/short2.vvc");
    let _ = decoder.decode(SECOND_DATA, None, None, false);
    let first_frame = decoder.flush().unwrap();
    assert_eq!(first_frame.width(), 160);
    assert_eq!(first_frame.height(), 120);

    Ok(())
}
