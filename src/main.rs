use anyhow::Result;
use clap::Parser;
use rand::Rng;
use rstonie::decode_encode;
use rstonie::Args;
use std::fs::File;

use toniefile::Toniefile;
fn main() -> Result<()> {
    let args = Args::parse();
    let outfile = File::create(&args.output)?;

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
    decode_encode(&args.input, &mut toniefile)?;

    // add additional tracks to the toniefile
    for extra_input in args.extra_inputs.clone().unwrap_or_default() {
        toniefile.new_chapter()?;
        decode_encode(&extra_input, &mut toniefile)?;
    }

    println!("all done");
    toniefile.finalize()?;
    println!("Toniefile written to {}", args.output.display());
    println!("kkthxbye");
    Ok(())
}
