use anyhow::Result;
use clap::Parser;
use rand::Rng;
use rstoni::resampler::Resampler;
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

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input file
    input: PathBuf,
    /// Output file
    output: PathBuf,
    /// Optional user comment to be added to the output file
    #[clap(short = 'c', long)]
    comments: Option<Vec<String>>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    // Open the media source.
    let src = std::fs::File::open(&args.input)?;
    // Create the media source stream.
    let mss = MediaSourceStream::new(Box::new(src), Default::default());
    // if the input file has an extension, use it as a hint for the media format.
    let mut hint = Hint::new();
    if let Some(ext) = args.input.extension() {
        if let Some(ext) = ext.to_str() {
            hint.with_extension(ext);
        }
    }
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();
    // Probe the media source.
    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;
    // Get the instantiated format reader.
    let mut format = probed.format;
    // Find the first audio track with a known (decodeable) codec.
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(anyhow::anyhow!("No supported audio track found"))?;
    // Use the default options for the decoder.
    let dec_opts: DecoderOptions = Default::default();
    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;
    // Store the track identifier, it will be used to filter packets.
    let track_id = track.id;

    let mut rnd = rand::thread_rng();
    let audio_id = rnd.gen::<u32>();
    let file = File::create(args.output)?;
    // let strcomments: Option<Vec<&str>>;
    // if let Some(comments) = args.comments {
    //     // let c = comments.iter().map(AsRef::as_ref).collect();
    //     strcomments = Some(comments.iter().map(AsRef::as_ref).collect());
    // } else {
    //     strcomments = None;
    // }
    let mut toniefile = Toniefile::new(file, audio_id, None)?;
    let input_sample_rate = track.codec_params.sample_rate.unwrap();
    let input_channels = track.codec_params.channels.unwrap().count(); //TODO
    // let input_bitrate = track.codec_params.bits_per_sample.unwrap();
    println!(
        "Input file: {} Hz, {} channels",
        input_sample_rate,
        input_channels,
    );

    // let signalspec = SignalSpec::new(track.codec_params.sample_rate.unwrap(), track.codec_params.channels.unwrap());

    let mut resampler: Option<Resampler<i16>> = None;
    // The decode loop.
    while let Ok(packet) = format.next_packet() {
        // Consume any new metadata that has been read since the last packet.
        while !format.metadata().is_latest() {
            // Pop the old head of the metadata queue.
            format.metadata().pop();

            // Consume the new metadata at the head of the metadata queue.
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
                // let resampled = resampler.resample(decoded.clone()).unwrap();
                if let Some(resampled) = resampler.as_mut().unwrap().resample(decoded.clone()) {
                    toniefile
                        .encode(resampled)?;
                }
                // let mut sample_buf =
                    // SampleBuffer::<i16>::new(decoded.capacity() as u64, *decoded.spec());
                // sample_buf.copy_interleaved_ref(decoded);
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
    toniefile.finalize()?;
    Ok(())
}


