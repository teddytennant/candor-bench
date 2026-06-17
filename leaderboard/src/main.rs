//! CLI entry point: read the canonical result JSON and write a static
//! `index.html` + `style.css`. Mirrors `scripts/plot_results.py`'s argparse
//! surface; errors print `error: ...` to stderr and exit non-zero.

use std::path::PathBuf;
use std::process;

use candor_leaderboard::canonical::{EVALS, MODELS};
use candor_leaderboard::data::{load_all, rank_axis};
use candor_leaderboard::render::{render_page, style_css};

const HELP: &str = "\
leaderboard — render candor-bench result JSON into a minimal B&W HTML leaderboard

USAGE:
    leaderboard [--results-dir DIR] [--out-dir DIR] [--title TITLE]

OPTIONS:
    --results-dir DIR   directory of canonical EvalReport JSON files (default: results)
    --out-dir DIR       output directory for index.html + style.css (default: site)
    --title TITLE       page <title> and <h1> text
    -h, --help          show this help
";

struct Args {
    results_dir: PathBuf,
    out_dir: PathBuf,
    title: String,
}

fn parse_args() -> Result<Args, String> {
    let mut results_dir = PathBuf::from("results");
    let mut out_dir = PathBuf::from("site");
    let mut title = String::from("candor-bench: per-axis honesty leaderboard");

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                process::exit(0);
            }
            "--results-dir" => {
                results_dir = it.next().ok_or("--results-dir needs a value")?.into();
            }
            "--out-dir" => {
                out_dir = it.next().ok_or("--out-dir needs a value")?.into();
            }
            "--title" => {
                title = it.next().ok_or("--title needs a value")?;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Args {
        results_dir,
        out_dir,
        title,
    })
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    if !args.results_dir.is_dir() {
        return Err(format!(
            "results dir not found: {}",
            args.results_dir.display()
        ));
    }

    let cells = load_all(&args.results_dir)?;
    let html = render_page(&cells, &args.title);

    std::fs::create_dir_all(&args.out_dir)
        .map_err(|e| format!("cannot create {}: {e}", args.out_dir.display()))?;
    let index = args.out_dir.join("index.html");
    let css = args.out_dir.join("style.css");
    std::fs::write(&index, html).map_err(|e| format!("cannot write {}: {e}", index.display()))?;
    std::fs::write(&css, style_css())
        .map_err(|e| format!("cannot write {}: {e}", css.display()))?;

    println!("wrote {} and {}", index.display(), css.display());
    println!(
        "  {} cells | {} evals | {} models",
        cells.len(),
        EVALS.len(),
        MODELS.len()
    );
    for eval in EVALS {
        let order = rank_axis(eval, &cells)
            .iter()
            .map(|r| format!("{}.{} ({:.3})", r.rank, r.label, r.score))
            .collect::<Vec<_>>()
            .join("  ");
        println!("  {eval:12} {order}");
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
