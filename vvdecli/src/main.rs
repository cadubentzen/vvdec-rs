use std::{fs::File, io::Read, io::Write, path::PathBuf};

use clap::Parser;
use vvdec::{ColorFormat, Decoder, Error, Frame, Params, PlaneComponent};
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
    let mut params = Params::new();
    params.set_remove_padding(true);
    let mut decoder = Decoder::with_params(params).expect("could not open decoder");

    let mut y4m_encoder = None;
    while let Some(chunk) = chunked_reader.next_chunk()? {
        match decoder.decode(chunk, None, None, false) {
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

    loop {
        match decoder.flush() {
            Ok(frame) => {
                let y4m_encoder = y4m_encoder.get_or_insert_with(|| {
                    let writer = std::mem::replace(&mut writer, Box::new(std::io::sink()));
                    create_y4m_encoder(&frame, writer).expect("could not create y4m encoder")
                });
                write_frame(y4m_encoder, frame)?;
            }
            Err(Error::Eof) => break,
            Err(err) => return Err(err.into()),
        }
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

fn write_frame(encoder: &mut y4m::Encoder<impl Write>, frame: Frame) -> anyhow::Result<()> {
    encoder.write_frame(&y4m::Frame::new(
        [
            frame.plane(PlaneComponent::Y).as_ref(),
            frame.plane(PlaneComponent::U).as_ref(),
            frame.plane(PlaneComponent::V).as_ref(),
        ],
        None,
    ))?;
    Ok(())
}
