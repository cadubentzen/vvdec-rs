use std::{fs::File, io::Read, io::Write, path::PathBuf};

use clap::Parser;
use vvdec::{ColorFormat, Decoder, Error, Frame, PlaneComponent};
use y4m::{Colorspace, Encoder};

mod chunked_reader;
use chunked_reader::ChunkedReader;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Input VVC file. If empty, input is read from stdin.
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Output Y4M file. If empty, output is written to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let reader: Box<dyn Read> = cli.input.map_or(Box::new(std::io::stdin()), |i| {
        Box::new(File::open(i).expect("could not open input file"))
    });

    let mut writer: Box<dyn Write> = cli.output.map_or(Box::new(std::io::stdout()), |o| {
        Box::new(File::create(o).expect("could not open output file"))
    });

    let mut chunked_reader = ChunkedReader::new(reader);
    let mut decoder = Decoder::builder().build()?;

    let mut y4m_encoder = None;
    while let Some(chunk) = chunked_reader.next_chunk()? {
        match decoder.decode(chunk) {
            Ok(Some(frame)) => {
                let y4m_encoder = y4m_encoder.get_or_insert_with(|| {
                    let writer = std::mem::replace(&mut writer, Box::new(std::io::sink()));
                    create_y4m_encoder(&frame, writer).expect("could not create y4m encoder")
                });
                write_frame(y4m_encoder, frame)?;
            }
            Ok(None) | Err(Error::TryAgain) => {}
            Err(err) => return Err(err.into()),
        }
    }

    while let Some(frame) = decoder.flush()? {
        let y4m_encoder = y4m_encoder.get_or_insert_with(|| {
            let writer = std::mem::replace(&mut writer, Box::new(std::io::sink()));
            create_y4m_encoder(&frame, writer).expect("could not create y4m encoder")
        });
        write_frame(y4m_encoder, frame)?;
    }

    Ok(())
}

fn create_y4m_encoder<W: Write>(frame: &Frame, writer: W) -> Result<Encoder<W>, y4m::Error> {
    let hrd = frame.picture_attributes().unwrap().hrd.unwrap();
    y4m::encode(
        frame.width() as usize,
        frame.height() as usize,
        y4m::Ratio {
            num: hrd.time_scale as usize,
            den: hrd.num_units_in_tick as usize,
        },
    )
    .with_colorspace(convert_colorspace(frame.color_format(), frame.bit_depth()))
    .write_header(writer)
}

fn convert_colorspace(color_format: ColorFormat, bit_depth: u32) -> Colorspace {
    if bit_depth > 8 {
        match color_format {
            ColorFormat::Yuv420Planar => Colorspace::C420p10,
            ColorFormat::Yuv422Planar => Colorspace::C422p10,
            ColorFormat::Yuv444Planar => Colorspace::C444p10,
            _ => unimplemented!(),
        }
    } else {
        match color_format {
            ColorFormat::Yuv420Planar => Colorspace::C420,
            ColorFormat::Yuv422Planar => Colorspace::C422,
            ColorFormat::Yuv444Planar => Colorspace::C444,
            _ => unimplemented!(),
        }
    }
}

fn remove_padding(frame: Frame) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let y_plane = frame.plane(PlaneComponent::Y).unwrap();
    let u_plane = frame.plane(PlaneComponent::U).unwrap();
    let v_plane = frame.plane(PlaneComponent::V).unwrap();

    let y_plane_data = extract_plane_data(&y_plane);
    let u_plane_data = extract_plane_data(&u_plane);
    let v_plane_data = extract_plane_data(&v_plane);

    (y_plane_data, u_plane_data, v_plane_data)
}

fn extract_plane_data(plane: &vvdec::Plane) -> Vec<u8> {
    let mut plane_data = Vec::with_capacity((plane.width() * plane.height() * plane.bytes_per_sample()) as usize);
    for row in 0..plane.height() {
        let start = (row * plane.stride()) as usize;
        let end = start + (plane.width() * plane.bytes_per_sample()) as usize;
        plane_data.extend_from_slice(&plane.as_ref()[start..end]);
    }
    plane_data
}

fn write_frame(encoder: &mut y4m::Encoder<impl Write>, frame: Frame) -> anyhow::Result<()> {
    let (y_plane, u_plane, v_plane) = remove_padding(frame);
    encoder.write_frame(&y4m::Frame::new(
        [
            y_plane.as_slice(), u_plane.as_slice(), v_plane.as_slice(),
        ],
        None,
    ))?;
    Ok(())
}
