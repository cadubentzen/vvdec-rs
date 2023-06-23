use std::{
    fs::File,
    io::{BufReader, Read, Write},
    path::PathBuf,
};

use clap::Parser;
use vvdec::{ColorFormat, Decoder, Error, Frame, Params};
use y4m::{Colorspace, Encoder};

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

// const INPUT_BUFFER_SIZE: usize = 8 * 1024 * 1024;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut reader: BufReader<Box<dyn Read>> =
        BufReader::new(cli.input.map_or(Box::new(std::io::stdin()), |i| {
            Box::new(File::open(i).expect("could not open input file"))
        }));

    let writer: Box<dyn Write> = cli.output.map_or(Box::new(std::io::stdout()), |o| {
        Box::new(File::create(o).expect("could not open output file"))
    });

    let mut input_buffer = Vec::new();

    // TODO: implement chunked reading on Annex-B start codes
    reader.read_to_end(&mut input_buffer)?;

    let mut params = Params::new();
    params.set_remove_padding(true);
    let mut decoder = Decoder::with_params(params).expect("could not open decoder");

    let first_frame = match decoder.decode(&input_buffer, None, None, false) {
        Ok(some_frame) => some_frame,
        Err(Error::TryAgain) => None,
        Err(err) => return Err(err.into()),
    };

    let first_frame = first_frame.unwrap_or_else(|| decoder.flush().unwrap());
    let mut y4m_encoder = create_y4m_encoder(&first_frame, writer)?;
    y4m_encoder.write_frame(&y4m::Frame::new(
        [
            first_frame.plane(0).unwrap().as_ref(),
            first_frame.plane(1).unwrap().as_ref(),
            first_frame.plane(2).unwrap().as_ref(),
        ],
        None,
    ))?;

    loop {
        match decoder.flush() {
            Ok(frame) => {
                y4m_encoder.write_frame(&y4m::Frame::new(
                    [
                        frame.plane(0).unwrap().as_ref(),
                        frame.plane(1).unwrap().as_ref(),
                        frame.plane(2).unwrap().as_ref(),
                    ],
                    None,
                ))?;
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
    .with_colorspace(convert_colorspace(frame.color_format()))
    .write_header(writer)
}

fn convert_colorspace(color_format: ColorFormat) -> Colorspace {
    match color_format {
        ColorFormat::Yuv420Planar => Colorspace::C420p10,
        ColorFormat::Yuv422Planar => Colorspace::C422p10,
        ColorFormat::Yuv444Planar => Colorspace::C444p10,
        _ => unimplemented!(),
    }
}
