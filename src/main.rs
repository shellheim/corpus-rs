use anyhow::{Context, Result, bail};
use clap::Parser;
use owo_colors::OwoColorize;
use quick_xml::Reader;
use quick_xml::events::Event;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::{thread, time::Duration};
use unicode_normalization::UnicodeNormalization;

// Repository and metadata definitions.

const REPOS: &[(&str, &str); 9] = &[
    (
        "adventures",
        "https://github.com/standardebooks/arthur-conan-doyle_the-adventures-of-sherlock-holmes.git",
    ),
    (
        "memoirs",
        "https://github.com/standardebooks/arthur-conan-doyle_the-memoirs-of-sherlock-holmes.git",
    ),
    (
        "return",
        "https://github.com/standardebooks/arthur-conan-doyle_the-return-of-sherlock-holmes.git",
    ),
    (
        "his-last-bow",
        "https://github.com/standardebooks/arthur-conan-doyle_his-last-bow.git",
    ),
    (
        "case-book",
        "https://github.com/standardebooks/arthur-conan-doyle_the-casebook-of-sherlock-holmes.git",
    ),
    (
        "study-in-scarlet",
        "https://github.com/standardebooks/arthur-conan-doyle_a-study-in-scarlet.git",
    ),
    (
        "sign-of-four",
        "https://github.com/standardebooks/arthur-conan-doyle_the-sign-of-the-four.git",
    ),
    (
        "hound-of-the-baskervilles",
        "https://github.com/standardebooks/arthur-conan-doyle_the-hound-of-the-baskervilles.git",
    ),
    (
        "valley-of-fear",
        "https://github.com/standardebooks/arthur-conan-doyle_the-valley-of-fear.git",
    ),
];

static COLLECTIONS: LazyLock<HashMap<&'static str, Vec<&'static str>>> = LazyLock::new(|| {
    HashMap::from([
        (
            "adventures",
            vec![
                "A Scandal in Bohemia",
                "The Red-Headed League",
                "A Case of Identity",
                "The Boscombe Valley Mystery",
                "The Five Orange Pips",
                "The Man with the Twisted Lip",
                "The Blue Carbuncle",
                "The Speckled Band",
                "The Engineer's Thumb",
                "The Noble Bachelor",
                "The Beryl Coronet",
                "The Copper Beeches",
            ],
        ),
        (
            "memoirs",
            vec![
                "Silver Blaze",
                "The Yellow Face",
                "The Stockbroker's Clerk",
                "The Gloria Scott",
                "The Musgrave Ritual",
                "The Reigate Puzzle",
                "The Crooked Man",
                "The Resident Patient",
                "The Greek Interpreter",
                "The Naval Treaty",
                "The Final Problem",
                "The Cardboard Box",
            ],
        ),
        (
            "return",
            vec![
                "The Empty House",
                "The Norwood Builder",
                "The Dancing Men",
                "The Solitary Cyclist",
                "The Priory School",
                "Black Peter",
                "Charles Augustus Milverton",
                "The Six Napoleons",
                "The Three Students",
                "The Golden Pince-Nez",
                "The Missing Three-Quarter",
                "The Abbey Grange",
                "The Second Stain",
            ],
        ),
        (
            "his-last-bow",
            vec![
                "Wisteria Lodge",
                "The Bruce-Partington Plans",
                "The Devil's Foot",
                "The Red Circle",
                "The Disappearance of Lady Frances Carfax",
                "The Dying Detective",
                "His Last Bow",
            ],
        ),
        (
            "case-book",
            vec![
                "The Mazarin Stone",
                "The Problem of Thor Bridge",
                "The Creeping Man",
                "The Sussex Vampire",
                "The Three Garridebs",
                "The Illustrious Client",
                "The Three Gables",
                "The Blanched Soldier",
                "The Lion's Mane",
                "The Retired Colourman",
                "The Veiled Lodger",
                "Shoscombe Old Place",
            ],
        ),
    ])
});

static STORY_YEARS: LazyLock<HashMap<&'static str, u32>> = LazyLock::new(|| {
    HashMap::from([
        ("A Scandal in Bohemia", 1891),
        ("The Red-Headed League", 1891),
        ("A Case of Identity", 1891),
        ("The Boscombe Valley Mystery", 1891),
        ("The Five Orange Pips", 1891),
        ("The Man with the Twisted Lip", 1891),
        ("The Blue Carbuncle", 1892),
        ("The Speckled Band", 1892),
        ("The Engineer's Thumb", 1892),
        ("The Noble Bachelor", 1892),
        ("The Beryl Coronet", 1892),
        ("The Copper Beeches", 1892),
        ("Silver Blaze", 1892),
        ("The Yellow Face", 1893),
        ("The Stockbroker's Clerk", 1893),
        ("The Gloria Scott", 1893),
        ("The Musgrave Ritual", 1893),
        ("The Reigate Puzzle", 1893),
        ("The Crooked Man", 1893),
        ("The Resident Patient", 1893),
        ("The Greek Interpreter", 1893),
        ("The Naval Treaty", 1893),
        ("The Final Problem", 1893),
        ("The Cardboard Box", 1893),
        ("The Empty House", 1903),
        ("The Norwood Builder", 1903),
        ("The Dancing Men", 1903),
        ("The Solitary Cyclist", 1903),
        ("The Priory School", 1904),
        ("Black Peter", 1904),
        ("Charles Augustus Milverton", 1904),
        ("The Six Napoleons", 1904),
        ("The Three Students", 1904),
        ("The Golden Pince-Nez", 1904),
        ("The Missing Three-Quarter", 1904),
        ("The Abbey Grange", 1904),
        ("The Second Stain", 1904),
        ("Wisteria Lodge", 1908),
        ("The Bruce-Partington Plans", 1908),
        ("The Devil's Foot", 1910),
        ("The Red Circle", 1911),
        ("The Disappearance of Lady Frances Carfax", 1911),
        ("The Dying Detective", 1913),
        ("His Last Bow", 1917),
        ("The Mazarin Stone", 1921),
        ("The Problem of Thor Bridge", 1922),
        ("The Creeping Man", 1923),
        ("The Sussex Vampire", 1924),
        ("The Three Garridebs", 1925),
        ("The Illustrious Client", 1925),
        ("The Three Gables", 1926),
        ("The Blanched Soldier", 1926),
        ("The Lion's Mane", 1926),
        ("The Retired Colourman", 1926),
        ("The Veiled Lodger", 1927),
        ("Shoscombe Old Place", 1927),
    ])
});

static NOVEL_YEARS: LazyLock<HashMap<&'static str, u32>> = LazyLock::new(|| {
    HashMap::from([
        ("study-in-scarlet", 1887),
        ("sign-of-four", 1890),
        ("hound-of-the-baskervilles", 1902),
        ("valley-of-fear", 1915),
    ])
});

static NOVEL_TITLES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        ("study-in-scarlet", "A Study in Scarlet"),
        ("sign-of-four", "The Sign of the Four"),
        ("hound-of-the-baskervilles", "The Hound of the Baskervilles"),
        ("valley-of-fear", "The Valley of Fear"),
    ])
});

fn ask_permission(prompt: &str) -> io::Result<bool> {
    loop {
        print!("{}? (Y/n): ", prompt);
        io::stdout().flush()?;

        let mut input = String::new();
        let bytes_read = io::stdin().read_line(&mut input)?;

        if bytes_read == 0 {
            println!("\nEOF detected. Aborting.");
            return Ok(false);
        }

        match input.trim().to_lowercase().as_str() {
            "y" | "yes" | "" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Invalid input. Please enter 'Y' or 'n'."),
        }
    }
}

/// Normalizes Unicode and cleans up typography/spacing mistakes.
fn normalize_text(text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");

    let text: String = text
        .nfkc()
        .filter(|c| {
            if matches!(*c, '\n' | '\t') {
                return true;
            }

            if matches!(
                c,
                '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'
            ) {
                return false;
            }

            if c.is_control() && *c != '\u{00A0}' {
                return false;
            }

            true
        })
        .collect();

    let text = text.replace('\u{00A0}', " ");
    let text = text.replace('\u{00AD}', "");

    static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+").unwrap());
    static LINE_BREAKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" *\n *").unwrap());
    static BLANK_LINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());

    let text = SPACES.replace_all(&text, " ");
    let text = LINE_BREAKS.replace_all(&text, "\n");
    let text = BLANK_LINES.replace_all(&text, "\n\n");

    text.trim().to_string()
}

fn slugify(text: &str) -> String {
    let text = text
        .nfkd()
        .filter(|c| !c.is_alphabetic() || c.is_ascii())
        .collect::<String>()
        .to_lowercase();

    let text = text.replace('’', "'");

    static SLUG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());

    SLUG_RE
        .replace_all(&text, "-")
        .trim_matches('-')
        .to_string()
}

fn title_key(text: &str) -> String {
    let text = text
        .nfkd()
        .filter(|c| !c.is_alphabetic() || c.is_ascii())
        .collect::<String>()
        .to_lowercase();

    let text = text.replace('’', "'");

    static ADV_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^the adventure of\s+").unwrap());
    let text = ADV_RE.replace_all(&text, "");

    static ALNUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());

    ALNUM_RE.replace_all(&text, "").to_string()
}

/// Parses XHTML and extracts text from block-level elements without heap allocations on every XML event.
fn extract_xhtml(path: &Path) -> Result<Vec<(String, String)>> {
    let source = fs::read_to_string(path)?;
    let mut reader = Reader::from_str(&source);

    let mut blocks = Vec::new();
    let mut block_stack: Vec<(String, String)> = Vec::new();
    let mut skip_depth: i32 = 0;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                let tag_bytes = name.as_ref();

                if matches!(tag_bytes, b"script" | b"style" | b"svg" | b"head") {
                    skip_depth += 1;
                } else if skip_depth == 0
                    && matches!(
                        tag_bytes,
                        b"p" | b"blockquote" | b"h1" | b"h2" | b"h3" | b"h4" | b"h5" | b"h6"
                    )
                {
                    let tag = String::from_utf8_lossy(tag_bytes).to_lowercase();
                    block_stack.push((tag, String::new()));
                }
            }

            Ok(Event::End(ref e)) => {
                let name = e.name();
                let tag_bytes = name.as_ref();

                if matches!(tag_bytes, b"script" | b"style" | b"svg" | b"head") {
                    skip_depth = skip_depth.saturating_sub(1);
                } else if skip_depth == 0
                    && matches!(
                        tag_bytes,
                        b"p" | b"blockquote" | b"h1" | b"h2" | b"h3" | b"h4" | b"h5" | b"h6"
                    )
                    && !block_stack.is_empty()
                {
                    let (block_tag, text) = block_stack.pop().unwrap();
                    let norm = normalize_text(&text);

                    if !norm.is_empty() {
                        blocks.push((block_tag, norm));
                    }
                }
            }

            Ok(Event::Text(e)) => {
                if skip_depth == 0 {
                    if let Some((_, text)) = block_stack.last_mut() {
                        text.push_str(&e.unescape().unwrap_or_default());
                    }
                }
            }

            Ok(Event::Eof) => break,

            Err(e) => bail!(
                "XML error at position {}: {:?}",
                reader.buffer_position(),
                e
            ),

            _ => {}
        }

        buf.clear();
    }

    Ok(blocks)
}

fn extract_xhtml_title(path: &Path) -> Result<Option<String>> {
    let source = fs::read_to_string(path)?;

    static TITLE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?is)<title\b[^>]*>(.*?)</title>").unwrap());
    static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

    if let Some(caps) = TITLE_RE.captures(&source) {
        let title = quick_xml::escape::unescape(&caps[1])?.into_owned();
        let title = TAG_RE.replace_all(&title, "").to_string();
        let norm = normalize_text(&title);

        Ok(if norm.is_empty() { None } else { Some(norm) })
    } else {
        Ok(None)
    }
}

fn extract_novel_subtitle(path: &Path) -> Result<Option<String>> {
    if let Some(title) = extract_xhtml_title(path)? {
        static ROMAN_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?i)^\s*[IVXLCDM]+\.?\s*:\s*").unwrap());

        let sub = ROMAN_RE.replace(&title, "").to_string();
        let sub = normalize_text(&sub);

        if sub != title {
            return Ok(Some(sub));
        }
    }

    Ok(None)
}

fn extract_novel_content(path: &Path) -> Result<(Option<String>, String)> {
    let subtitle = extract_novel_subtitle(path)?;
    let blocks = extract_xhtml(path)?;

    let mut prose_blocks: Vec<String> = blocks
        .into_iter()
        .filter(|(tag, _)| tag == "p" || tag == "blockquote")
        .map(|(_, text)| text)
        .collect();

    if let Some(ref sub) = subtitle {
        prose_blocks.retain(|block| block != sub);
    }

    let prose = normalize_text(&prose_blocks.join("\n\n"));

    if prose.is_empty() {
        bail!("No prose remains after subtitle extraction from {:?}", path);
    }

    Ok((subtitle, prose))
}

fn clone_or_update(url: &str, destination: &Path) -> Result<()> {
    if destination.join(".git").exists() {
        println!("{} {}", "Already present:".dimmed(), destination.display());
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    println!("\n{} {}", "Cloning:".bold(), destination.display());

    let status = Command::new("git")
        .args(["clone", "--depth", "1", url, destination.to_str().unwrap()])
        .status()?;

    if !status.success() {
        bail!("Failed to clone {}", url);
    }

    Ok(())
}

fn novel_chapter_key(path: &Path) -> Result<(u32, u32)> {
    static CHAPTER_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)chapter-(\d+)(?:-(\d+))?").unwrap());

    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    if let Some(caps) = CHAPTER_RE.captures(stem) {
        let part: u32 = caps[1].parse()?;
        let chapter: u32 = caps.get(2).map_or(1, |m| m.as_str().parse().unwrap());

        Ok((part, chapter))
    } else {
        bail!(
            "Could not determine chapter ordering from {:?}",
            path.file_name()
        )
    }
}

fn get_novel_chapters(repo: &Path) -> Result<Vec<PathBuf>> {
    let text_dir = repo.join("src/epub/text");

    if !text_dir.exists() {
        bail!("Missing source directory: {:?}", text_dir);
    }

    let mut files: Vec<PathBuf> = fs::read_dir(&text_dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            name.starts_with("chapter-") && (name.ends_with(".xhtml") || name.ends_with(".html"))
        })
        .collect();

    if files.is_empty() {
        bail!("No novel chapter files found in {:?}", text_dir);
    }

    files.sort_by_key(|p| novel_chapter_key(p).unwrap_or((0, 0)));

    Ok(files)
}

fn find_story(repo: &Path, wanted_title: &str) -> Result<(String, PathBuf)> {
    let wanted = title_key(wanted_title);
    let text_dir = repo.join("src/epub/text");

    let mut stack = vec![text_dir];
    let mut xhtml_files = Vec::new();

    while let Some(dir) = stack.pop() {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "xhtml") {
                    xhtml_files.push(path);
                }
            }
        }
    }

    xhtml_files.sort();

    for path in xhtml_files {
        let blocks = extract_xhtml(&path)?;

        let found = blocks.iter().any(|(tag, text)| {
            matches!(tag.as_str(), "h1" | "h2" | "h3") && title_key(text) == wanted
        });

        if found {
            let prose_blocks: Vec<String> = blocks
                .into_iter()
                .filter(|(tag, _)| matches!(tag.as_str(), "p" | "blockquote"))
                .map(|(_, text)| text)
                .collect();

            let prose = normalize_text(&prose_blocks.join("\n\n"));

            if !prose.is_empty() {
                return Ok((prose, path));
            }
        }
    }

    bail!("Could not find story {:?} in {:?}", wanted_title, repo)
}

fn yaml_quote(val: &str) -> String {
    format!("\"{}\"", val.replace('\\', "\\\\").replace('"', "\\\""))
}

fn story_frontmatter(title: &str, collection: &str, year: u32, release_order: u32) -> String {
    let display_title = title.strip_prefix("The Adventure of ").unwrap_or(title);

    format!(
        "---\ntitle: {}\ncollection: {}\nyear: {}\nrelease_order: {}\nlayout: \"single\"\n---\n\n",
        yaml_quote(display_title),
        yaml_quote(collection),
        year,
        release_order
    )
}

fn novel_frontmatter(
    title: &str,
    subtitle: Option<&str>,
    novel: &str,
    chapter: u32,
    year: u32,
) -> String {
    let mut out = format!("---\ntitle: {}\n", yaml_quote(title));

    if let Some(sub) = subtitle {
        out.push_str(&format!("subtitle: {}\n", yaml_quote(sub)));
    }

    out.push_str(&format!(
        "novel: {}\nchapter: {}\nyear: {}\nlayout: \"single\"\n---\n\n",
        yaml_quote(novel),
        chapter,
        year
    ));

    out
}

fn relative_content_path(path: &Path, content: &Path) -> String {
    let relative = path.strip_prefix(content).unwrap_or(path);
    format!("/{}", relative.to_string_lossy())
}

fn cleanup_created_tools(newly_cloned: &Arc<Mutex<Vec<PathBuf>>>) {
    if let Ok(guard) = newly_cloned.lock() {
        for repo_path in guard.iter() {
            if repo_path.exists() {
                let _ = fs::remove_dir_all(repo_path);
            }
        }
    }
}

fn handle_interrupt(newly_cloned: &Arc<Mutex<Vec<PathBuf>>>, removing_content: bool) {
    eprintln!();

    if removing_content {
        eprintln!(
            "{}",
            "WARNING: Interrupted while removing old content. Some files may already have been deleted."
                .red()
                .bold()
        );
    } else {
        eprintln!(
            "{}",
            "Interrupted. Cleaning up repositories created during this run...".yellow()
        );
    }

    thread::sleep(Duration::from_secs(2));

    cleanup_created_tools(newly_cloned);

    eprintln!("{}", "Exiting.".bold());

    std::process::exit(130);
}

fn install_ctrl_c_handler(
    newly_cloned: Arc<Mutex<Vec<PathBuf>>>,
    removing_content: Arc<AtomicBool>,
) -> Result<()> {
    ctrlc::set_handler(move || {
        let removing = removing_content.load(Ordering::SeqCst);
        handle_interrupt(&newly_cloned, removing);
    })?;

    Ok(())
}

#[derive(Parser)]
#[command(about = "Freshly rebuild the Sherlock Holmes Hugo corpus from Standard Ebooks.")]
struct Cli {
    #[arg(long, default_value = "content")]
    content: PathBuf,

    #[arg(long, default_value = "tools/sherlock")]
    tools: PathBuf,

    #[arg(long)]
    keep_old_content: bool,
}

fn main() -> Result<()> {
    let hugo_configs = ["hugo.toml", "hugo.yaml", "hugo.yml", "hugo.json"];

    if !hugo_configs.iter().any(|file| Path::new(file).exists()) {
        println!("No Hugo config found");
        println!("{}", "This is probably NOT a Hugo project".yellow());

        if !ask_permission("\nDo you still want to run")? {
            println!("\nExiting...");
            return Ok(());
        }
    }

    let args = Cli::parse();

    let root = std::env::current_dir()?;
    let content = root.join(&args.content);
    let tools = root.join(&args.tools);

    if !content.is_dir() && content.exists() {
        bail!("Content path is not a directory");
    }

    let newly_cloned = Arc::new(Mutex::new(Vec::new()));
    let removing_content = Arc::new(AtomicBool::new(false));

    install_ctrl_c_handler(Arc::clone(&newly_cloned), Arc::clone(&removing_content))?;

    let missing_repos = REPOS
        .iter()
        .filter(|(name, _)| !tools.join(name).join(".git").exists())
        .count();

    if missing_repos > 0 {
        println!(
            "\n{} {} source repositories are missing.",
            "NOTE:".bold(),
            missing_repos
        );

        if !ask_permission("Do you want to clone them")? {
            println!("\nExiting...");
            return Ok(());
        }
    }

    println!("\n{}\n", "=== SYNCING REPOSITORIES ===".bold());

    for (name, url) in REPOS {
        let dest = tools.join(name);
        if !dest.join(".git").exists() {
            newly_cloned.lock().unwrap().push(dest.clone());
        }
        clone_or_update(url, &dest)?;
    }

    if !args.keep_old_content && content.exists() {
        println!();

        println!(
            "{}",
            "WARNING: Existing generated content files will be DELETED."
                .red()
                .bold()
        );

        println!(
            "{}",
            "The deleted files will then be REPLACED with freshly generated files."
                .red()
                .bold()
        );

        println!("{}", "Existing _index.md files will be preserved.".yellow());

        println!();

        if !ask_permission("Do you want to continue")? {
            println!("\nExiting...");
            return Ok(());
        }

        println!("\n{}\n", "=== CLEARING OLD CONTENT ===".bold());

        removing_content.store(true, Ordering::SeqCst);

        let mut stack = vec![content.clone()];

        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir)? {
                let path = entry?.path();

                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "md")
                    && path.file_name().is_some_and(|name| name != "_index.md")
                {
                    println!(
                        "  {} {}",
                        "Removing:".red().bold(),
                        relative_content_path(&path, &content)
                    );

                    fs::remove_file(path)?;
                }
            }
        }

        removing_content.store(false, Ordering::SeqCst);
    }

    // Stories.
    println!("\n{}", "=== BUILDING STORIES ===".bold());

    let mut release_order = 0;

    let coll_order = [
        "adventures",
        "memoirs",
        "return",
        "his-last-bow",
        "case-book",
    ];

    for coll_name in coll_order {
        let titles = COLLECTIONS.get(coll_name).unwrap();

        let collection_title = match coll_name {
            "adventures" => "The Adventures of Sherlock Holmes",
            "memoirs" => "The Memoirs of Sherlock Holmes",
            "return" => "The Return of Sherlock Holmes",
            "his-last-bow" => "His Last Bow",
            "case-book" => "The Case-Book of Sherlock Holmes",
            _ => coll_name,
        };

        let heading = format!("{}: {} Stories", collection_title, titles.len());

        println!("\n{}", heading.bold());
        println!("{}", "─".repeat(heading.chars().count()).dimmed());

        let repo = tools.join(coll_name);
        let out_dir = content.join("stories").join(coll_name);

        fs::create_dir_all(&out_dir)?;

        for story_title in titles {
            release_order += 1;

            let year = STORY_YEARS
                .get(story_title)
                .context(format!("Missing year for {story_title}"))?;

            let (prose, source) = find_story(&repo, story_title)?;

            let filename = format!("{}.md", slugify(story_title));

            let output = out_dir.join(&filename);

            let fm = story_frontmatter(story_title, coll_name, *year, release_order);

            fs::write(&output, fm + &prose)?;

            println!(
                "  {} -> {}",
                source.file_name().unwrap().to_string_lossy().cyan(),
                relative_content_path(&output, &content).blue()
            );
        }
    }

    // Novels.
    println!("\n{}", "=== BUILDING NOVELS ===".bold());

    let novel_order = [
        "study-in-scarlet",
        "sign-of-four",
        "hound-of-the-baskervilles",
        "valley-of-fear",
    ];

    for novel_slug in novel_order {
        let novel_title = NOVEL_TITLES[novel_slug];
        let repo = tools.join(novel_slug);
        let out_dir = content.join("novels").join(novel_slug);

        fs::create_dir_all(&out_dir)?;

        let chapters = get_novel_chapters(&repo)?;

        let heading = format!("{}: {} Chapters", novel_title, chapters.len());

        println!("\n{}", heading.bold());
        println!("{}", "─".repeat(heading.chars().count()).dimmed());

        for (i, source) in chapters.iter().enumerate() {
            let hugo_number = (i + 1) as u32;

            let (source_part, source_chapter) = novel_chapter_key(source)?;

            let (subtitle, prose) = extract_novel_content(source)?;

            let filename = format!("chapter-{:02}.md", hugo_number);

            let output = out_dir.join(&filename);

            let fm = novel_frontmatter(
                &format!("Chapter {hugo_number}"),
                subtitle.as_deref(),
                novel_title,
                hugo_number,
                NOVEL_YEARS[novel_slug],
            );

            fs::write(&output, fm + &prose)?;

            let sub_print = subtitle.unwrap_or_else(|| "NO SUBTITLE".to_string());

            println!(
                "  {} [{}] [{}] -> {}",
                source.file_name().unwrap().to_string_lossy().cyan(),
                format!("Part {source_part}, Chapter {source_chapter}").dimmed(),
                sub_print.dimmed(),
                relative_content_path(&output, &content).red()
            );
        }
    }

    println!("\n{}", "Corpus rebuild complete.".green().bold());

    Ok(())
}
