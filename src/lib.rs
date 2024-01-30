use anyhow::Result;
use clap::Parser;
use std::path::Path;
use std::{fs::File, path::PathBuf};
use symphonia::core::errors::Error;
use symphonia::core::{
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};
use toniefile::Toniefile;

use crate::resampler::Resampler;

pub mod resampler;

pub fn get_styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .usage(
            anstyle::Style::new()
                .bold()
                .underline()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Yellow))),
        )
        .header(
            anstyle::Style::new()
                .bold()
                .underline()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Yellow))),
        )
        .literal(
            anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green))),
        )
        .invalid(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red))),
        )
        .error(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red))),
        )
        .valid(
            anstyle::Style::new()
                .bold()
                .underline()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green))),
        )
        .placeholder(
            anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::White))),
        )
}
use clap::Args;
#[derive(Parser)]
#[command(author, version, about, long_about = None, styles=get_styles())]
pub struct Cli {
    #[command(flatten)]
    pub input_args: InputArgs,
    #[command(flatten)]
    pub output_args: OutputArgs,
    /// Additional files to be added to the output file, in the order they are passed
    #[clap(short = 'a', long = "add", requires = "out")]
    pub extra_inputs: Option<Vec<PathBuf>>,
    /// Optional user comment to be added to the output file, can be passed multiple times
    #[clap(short = 'c', long, requires = "out")]
    pub comments: Option<Vec<String>>,
}

#[derive(Args)]
#[group(required = true)]
pub struct InputArgs {
    /// Input file (ogg, mp3... anything that the Rust Symphonia crate supports)
    pub input: PathBuf,
}

#[derive(Args)]
#[group(required = true)]
pub struct OutputArgs {
    /// Output file
    #[arg(group = "out")]
    pub output: Option<PathBuf>,
    /// Dump the header of the input file and exit
    #[clap(short = 'd', long = "header", group = "out")]
    pub dump_header: bool,

}

pub fn decode_encode(
    src: &Path,
    toniefile: &mut Toniefile<File>,
) -> Result<()> {
    println!("Encoding input file: {}", src.display());

    // if the input file has an extension, use it as a hint for the media format.
    let mut hint = Hint::new();
    if let Some(ext) = src.extension() {
        if let Some(ext) = ext.to_str() {
            hint.with_extension(ext);
        }
    }

    let src = std::fs::File::open(src)?;

    // Create the media source stream.
    let mss = MediaSourceStream::new(Box::new(src), Default::default());


    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    // Probe the media source.
    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

    // Get the instantiated format reader.
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(anyhow::anyhow!("No supported audio track found"))?;

    // Create a decoder for the track.
    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

    // Store the track identifier, it will be used to filter packets.
    let track_id = track.id;

    // create a resampler to convert to 48kHz
    let mut resampler: Option<Resampler<i16>> = None;

    let input_sample_rate = track.codec_params.sample_rate.unwrap_or_default();
    let input_channels = track.codec_params.channels.unwrap_or_default().count(); //TODO
    let tracklen = track.codec_params.n_frames.unwrap_or_default();

    // print some file info
    println!(
        "Input file: {} Hz, {} channels",
        input_sample_rate, input_channels,
    );

    println!("Track length: {} frames", tracklen);

    while let Ok(packet) = format.next_packet() {
        let progress = packet.ts * 100 / tracklen;
        print!("\rProgress: {}%", progress);
        // Consume any new metadata that has been read since the last packet.
        while !format.metadata().is_latest() {
            format.metadata().pop();
        }

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples.
        match decoder.decode(&packet) {
            Ok(decoded) => {
                // The packet was successfully decoded, process the audio samples.
                if resampler.is_none() {
                    resampler = Some(Resampler::new(
                        *decoded.spec(),
                        48000,
                        decoded.capacity() as u64 / 2,
                    ));
                }
                if let Some(resampled) = resampler.as_mut().unwrap().resample(decoded.clone()) {
                    toniefile.encode(resampled)?;
                }
            }
            Err(Error::IoError(_)) => {
                // The packet failed to decode due to an IO error, skip the packet.
                println!("IO error");
                continue;
            }
            Err(Error::DecodeError(_)) => {
                // The packet failed to decode due to invalid data, skip the packet.
                println!("Decode error");
                continue;
            }
            Err(err) => {
                // An unrecoverable error occured, halt decoding.
                panic!("{}", err);
            }
        }
    }
    println!("\rProgress: 100%");
    println!("File done");
    Ok(())
}
