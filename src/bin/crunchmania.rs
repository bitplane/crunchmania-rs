use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use crunchmania::constants::{ALL_MAGICS, HEADER_SIZE};
use crunchmania::{pack, parse_header, unpack};

#[derive(Parser)]
#[command(name = "crunchmania", about = "Crunch-Mania decompression tool")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// compress a file
    #[command(alias = "p")]
    Pack {
        input: PathBuf,
        output: Option<PathBuf>,
        #[arg(long, help = "use delta encoding")]
        sampled: bool,
    },
    /// decompress a file
    #[command(alias = "u")]
    Unpack {
        input: PathBuf,
        output: Option<PathBuf>,
    },
    /// show file info
    #[command(alias = "i")]
    Info { input: PathBuf },
    /// scan for embedded CrM blocks
    #[command(alias = "s")]
    Scan { input: PathBuf },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Pack {
            input,
            output,
            sampled,
        } => cmd_pack(&input, output.as_deref(), sampled),
        Command::Unpack { input, output } => cmd_unpack(&input, output.as_deref()),
        Command::Info { input } => cmd_info(&input),
        Command::Scan { input } => cmd_scan(&input),
    }
}

fn read_file(p: &Path) -> Result<Vec<u8>, ExitCode> {
    std::fs::read(p).map_err(|e| {
        eprintln!("error: cannot read {}: {}", p.display(), e);
        ExitCode::from(1)
    })
}

fn cmd_pack(input: &Path, output: Option<&Path>, sampled: bool) -> ExitCode {
    let data = match read_file(input) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let result = pack(&data, sampled);

    let output_path = match output {
        Some(o) => o.to_path_buf(),
        None => {
            let candidate = input.with_extension("crm");
            if candidate == input {
                let mut s = input.as_os_str().to_os_string();
                s.push(".crm.packed");
                PathBuf::from(s)
            } else {
                candidate
            }
        }
    };

    if let Err(e) = std::fs::write(&output_path, &result) {
        eprintln!("error: cannot write {}: {}", output_path.display(), e);
        return ExitCode::from(1);
    }
    println!(
        "packed {} -> {} bytes to {}",
        data.len(),
        result.len(),
        output_path.display()
    );
    ExitCode::SUCCESS
}

fn cmd_unpack(input: &Path, output: Option<&Path>) -> ExitCode {
    let data = match read_file(input) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let result = match unpack(&data) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::from(1);
        }
    };

    let output_path = match output {
        Some(o) => o.to_path_buf(),
        None => {
            let candidate = input.with_extension("");
            if candidate == input || candidate.as_os_str().is_empty() {
                let mut s = input.as_os_str().to_os_string();
                s.push(".unpacked");
                PathBuf::from(s)
            } else {
                candidate
            }
        }
    };

    if let Err(e) = std::fs::write(&output_path, &result) {
        eprintln!("error: cannot write {}: {}", output_path.display(), e);
        return ExitCode::from(1);
    }
    println!(
        "unpacked {} -> {} bytes to {}",
        data.len(),
        result.len(),
        output_path.display()
    );
    ExitCode::SUCCESS
}

fn cmd_info(input: &Path) -> ExitCode {
    let data = match read_file(input) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let header = match parse_header(&data) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::from(1);
        }
    };

    let mode = if header.is_lzh { "LZH" } else { "standard" };
    let delta = if header.is_sampled { " + delta" } else { "" };
    println!("file:          {}", input.display());
    // Match Python's `bytes!r` style: b'CrM!'
    println!("magic:         b'{}'", escape_magic(&header.magic));
    println!("mode:          {}{}", mode, delta);
    println!("packed size:   {}", header.packed_size);
    println!("unpacked size: {}", header.unpacked_size);
    let ratio = if header.unpacked_size > 0 {
        header.packed_size as f64 / header.unpacked_size as f64 * 100.0
    } else {
        0.0
    };
    println!("ratio:         {:.1}%", ratio);
    ExitCode::SUCCESS
}

fn escape_magic(magic: &[u8; 4]) -> String {
    let mut s = String::new();
    for &b in magic {
        if (0x20..0x7f).contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("\\x{:02x}", b));
        }
    }
    s
}

fn cmd_scan(input: &Path) -> ExitCode {
    let data = match read_file(input) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let mut found = 0usize;

    if data.len() >= HEADER_SIZE {
        for i in 0..=data.len() - HEADER_SIZE {
            let four: [u8; 4] = data[i..i + 4].try_into().unwrap();
            if !ALL_MAGICS.contains(&four) {
                continue;
            }
            if let Ok(header) = parse_header(&data[i..]) {
                let mode = if header.is_lzh { "LZH" } else { "std" };
                let delta = if header.is_sampled { "+delta" } else { "" };
                println!(
                    "  offset 0x{:08X}: {}{}, {} -> {} bytes",
                    i, mode, delta, header.packed_size, header.unpacked_size
                );
                found += 1;
            }
        }
    }

    if found == 0 {
        println!("no CrM data found");
    } else {
        println!("{} CrM block(s) found", found);
    }
    ExitCode::SUCCESS
}
