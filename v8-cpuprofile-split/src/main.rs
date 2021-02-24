#![deny(clippy::all, clippy::pedantic)]
#![feature(once_cell)]

use memmap::Mmap;
use std::fs::create_dir_all;
use std::fs::File;
use std::io::BufWriter;
use std::lazy::OnceCell;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use structopt::StructOpt;
use v8_cpuprofile::Profile;
use v8_cpuprofile::ProfileChunk;

#[derive(Debug, StructOpt)]
#[structopt(name = "cpuprofile-split")]
struct Opt {
    #[structopt(parse(from_os_str))]
    cpu_profile: PathBuf,
    #[structopt(parse(from_os_str))]
    out_dir: PathBuf,
    chunk_num: usize,
}

type Error = Box<dyn std::error::Error + Send + Sync>;

// since we serialize out in multiple threads and each chunk
// borrows from the profile and the profile borrows from mmap
// we just want to use a static to make it simple to move the
// chunk into the thread.
fn parse_cpuprofile(path: &Path) -> Result<&'static Profile<'static>, Error> {
    static mut MMAP: OnceCell<Mmap> = OnceCell::new();
    static mut PROFILE: OnceCell<Profile> = OnceCell::new();

    let file = File::open(path)?;
    let mmap = unsafe { MMAP.get_or_try_init(|| Mmap::map(&file))? };
    Ok(unsafe { PROFILE.get_or_try_init(|| serde_json::from_slice(mmap))? })
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();
    println!("parsing cpuprofile from {:?}", &opt.cpu_profile);
    let profile = parse_cpuprofile(&opt.cpu_profile)?;
    create_dir_all(&opt.out_dir)?;

    let results = Arc::new(Mutex::new(Vec::with_capacity(opt.chunk_num)));

    rayon::scope(|s| {
        for (index, chunk) in profile.chunks(opt.chunk_num).enumerate() {
            let results = results.clone();
            let mut path = opt.out_dir.clone();
            let num = index + 1;
            path.push(format!("part{}.cpuprofile", num));
            s.spawn(move |_| {
                let result = serialize_chunk(&chunk, &path, num);
                results.lock().unwrap().push(result);
            })
        }
    });

    for result in results.lock().unwrap().drain(..) {
        result?;
    }
    Ok(())
}

fn serialize_chunk(chunk: &ProfileChunk<'_, '_>, path: &Path, num: usize) -> Result<(), Error> {
    println!("writing chunk {} to {:?}", num, path);
    serde_json::to_writer(BufWriter::new(File::create(path)?), chunk)?;
    println!("chunk {} done", num);
    Ok(())
}
