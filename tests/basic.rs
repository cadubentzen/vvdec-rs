use vvdec::*;

macro_rules! assert_matches {
    ($a:expr, $b:pat) => {
        assert!(matches!($a, $b));
    };
}

const DATA: &[u8] = include_bytes!("../tests/short.vvc");

#[test]
fn basic() -> Result<(), Error> {
    let mut decoder = Decoder::builder().remove_padding(true).build()?;

    assert_matches!(decoder.decode(DATA), Err(Error::TryAgain));

    let frame1 = decoder.flush()?.unwrap();
    let _plane = frame1.plane(PlaneComponent::Y).unwrap();
    let _plane = frame1.plane(PlaneComponent::U).unwrap();
    let _plane = frame1.plane(PlaneComponent::V).unwrap();

    let _frame2 = decoder.flush()?.unwrap();
    let _frame3 = decoder.flush()?.unwrap();

    assert_matches!(decoder.flush(), Ok(None));
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
    let mut decoder = Decoder::new()?;

    for slice in split_data(DATA) {
        let _ = decoder.decode(slice);
    }

    let frame1 = decoder.flush()?.unwrap();
    let _plane = frame1.plane(PlaneComponent::Y).unwrap();
    let _plane = frame1.plane(PlaneComponent::U).unwrap();
    let _plane = frame1.plane(PlaneComponent::V).unwrap();

    let _frame2 = decoder.flush()?.unwrap();
    let _frame3 = decoder.flush()?.unwrap();

    assert_matches!(decoder.flush(), Ok(None));
    assert_matches!(decoder.flush(), Err(Error::RestartRequired));

    Ok(())
}

#[test]
fn test_decode_after_flush() -> Result<(), Error> {
    let mut decoder = Decoder::new()?;

    let mut slices = split_data(DATA).into_iter();
    let sps = slices.next().unwrap();
    let pps = slices.next().unwrap();
    let frame1 = slices.next().unwrap();
    let frame2 = slices.next().unwrap();
    let frame3 = slices.next().unwrap();

    let _ = decoder.decode(sps);
    let _ = decoder.decode(pps);
    let _ = decoder.decode(frame1);
    let _ = decoder.decode(frame2);

    assert!(decoder.flush()?.is_some());
    assert!(decoder.flush()?.is_some());
    assert!(decoder.flush()?.is_none());

    let _ = decoder.decode(sps);
    let _ = decoder.decode(pps);
    let _ = decoder.decode(frame1);
    let _ = decoder.decode(frame2);
    let _ = decoder.decode(frame3);
    assert!(decoder.flush()?.is_some());
    assert!(decoder.flush()?.is_some());
    assert!(decoder.flush()?.is_some());
    assert!(decoder.flush()?.is_none());

    Ok(())
}

#[test]
fn test_change_resolution() -> Result<(), Error> {
    let mut decoder = Decoder::new()?;

    let _ = decoder.decode(DATA);
    let first_frame = decoder.flush()?.unwrap();
    assert_eq!(first_frame.width(), 320);
    assert_eq!(first_frame.height(), 240);

    const SECOND_DATA: &[u8] = include_bytes!("../tests/short2.vvc");
    let _ = decoder.decode(SECOND_DATA);
    let first_frame = decoder.flush()?.unwrap();
    assert_eq!(first_frame.width(), 160);
    assert_eq!(first_frame.height(), 120);

    Ok(())
}
