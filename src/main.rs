use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use rand::Rng;
use rstonie::decode_encode;
use rstonie::Cli;
use rstonie::TonieboxAudioFileHeaderWrapper;
use std::fs::File;
use toniefile::Toniefile;

fn main() -> Result<()> {
    let args = Cli::parse();

    if args.output_args.dump_header {
        println!("dumping header of {}", args.input_args.input.display());
        let mut src = std::fs::File::open(&args.input_args.input)?;
        let header = TonieboxAudioFileHeaderWrapper(Toniefile::parse_header(&mut src)?);
        println!("{}", header);
        return Ok(());
    }
    let output_path = args
        .output_args
        .output
        .context("Output file not provided")?;
    let outfile = File::create(&output_path)?;

    // create a Toniefile to write to later
    let mut rnd = rand::thread_rng();
    let audio_id = rnd.gen::<u32>();
    let comments = args.comments.clone().unwrap_or_default();
    let strcomments: Option<Vec<&str>> = if comments.is_empty() {
        None
    } else {
        Some(comments.iter().map(AsRef::as_ref).collect())
    };

    let mut toniefile = Toniefile::new(outfile, audio_id, strcomments)?;

    if !comments.is_empty() {
        for comment in comments {
            println!("ogg user comment: {}", comment);
        }
    }
    // decode the first file and encode it
    decode_encode(&args.input_args.input, &mut toniefile)?;

    // add additional tracks to the toniefile
    for extra_input in args.extra_inputs.clone().unwrap_or_default() {
        toniefile.new_chapter()?;
        decode_encode(&extra_input, &mut toniefile)?;
    }

    println!("all done");
    toniefile.finalize()?;
    println!("Toniefile written to {}", output_path.display());
    println!("kkthxbye");
    Ok(())
}
