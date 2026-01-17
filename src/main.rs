//! A basic example of a preprocessor that does nothing.

use crate::nop_lib::TranAI;
use clap::{Arg, ArgMatches, Command};
use mdbook_preprocessor::book::Book;
use mdbook_preprocessor::errors::Result;
use mdbook_preprocessor::{Preprocessor, PreprocessorContext};
use semver::{Version, VersionReq};
use std::io;
use std::process;

const TRAN_NAME: &str = "tranai";

fn make_app() -> Command {
    Command::new(TRAN_NAME)
        .about("Translation preprocessor using AI interfaces")
        .subcommand(
            Command::new("supports")
                .arg(Arg::new("renderer").required(true))
                .about("Check whether a renderer is supported by this preprocessor"),
        )
}

fn main() {
    let matches = make_app().get_matches();

    // Users will want to construct their own preprocessor here
    let preprocessor = TranAI::new();

    if let Some(sub_args) = matches.subcommand_matches("supports") {
        handle_supports(&preprocessor, sub_args);
    } else if let Err(e) = handle_preprocessing(&preprocessor) {
        eprintln!("{e:?}");
        process::exit(1);
    }
}

fn handle_preprocessing(pre: &dyn Preprocessor) -> Result<()> {
    let (ctx, book) = mdbook_preprocessor::parse_input(io::stdin())?;

    let book_version = Version::parse(&ctx.mdbook_version)?;
    let version_req = VersionReq::parse(mdbook_preprocessor::MDBOOK_VERSION)?;

    if !version_req.matches(&book_version) {
        eprintln!(
            "Warning: The {} plugin was built against version {} of mdbook, \
             but we're being called from version {}",
            pre.name(),
            mdbook_preprocessor::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let processed_book = pre.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &processed_book)?;

    Ok(())
}

fn handle_supports(pre: &dyn Preprocessor, sub_args: &ArgMatches) -> ! {
    let renderer = sub_args
        .get_one::<String>("renderer")
        .expect("Required argument");
    let supported = pre.supports_renderer(renderer).unwrap();

    // Signal whether the renderer is supported by exiting with 1 or 0.
    if supported {
        process::exit(0);
    } else {
        process::exit(1);
    }
}

/// The actual implementation of the `Nop` preprocessor. This would usually go
/// in your main `lib.rs` file.
#[allow(unreachable_pub, reason = "wouldn't be a problem in a proper lib.rs")]
mod nop_lib {
    use core::{fmt, panic};
    use std::{
        collections::HashSet,
        env::{self, current_dir},
        fs::File,
        io::{Read, Write},
        path::PathBuf,
        vec,
    };

    use gemini_rust::Part;
    use mdbook_preprocessor::book::BookItem;
    use reqwest::StatusCode;
    use serde::{Deserialize, Serialize};
    use serde_json::{self, json};
    use tokio::runtime::Runtime;
    use url::Url;

    use super::*;

    /// A no-op preprocessor.
    pub struct TranAI;
    struct GeminiConf {
        api_key: String,
        url: Option<Url>,
    }

    /// This will be used to fetching all sorts of data like file and http from requests.
    fn fetch_url(url: Url) -> String {
        match url.scheme() {
            "file" => {
                let mut dest = url.path();
                let mut opened =
                    File::open(dest).unwrap_or_else(|e| panic!("Could not open a file: {e}"));
                let mut buf = String::new();
                opened
                    .read_to_string(&mut buf)
                    .expect("couldn't read a file to the buffer");
                return buf;
            }
            "https" => {
                let res = reqwest::blocking::get(url);
                let res = res.unwrap_or_else(|e| panic!("Could not download text: {e}"));
                match res.status() {
                    StatusCode::OK => res
                        .text()
                        .unwrap_or_else(|e| panic!("Could not decode text after download: {e}")),
                    s => panic!("cannot download file from the internet: {s}"),
                }
            }
            other => {
                panic!("Can't parse this type of url: {}", other);
            }
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct ToTranslate {
        file_path: PathBuf,
        content: String,
        chapter_title: String,
    }

    fn get_cached_translations(opened_file: File) -> Vec<ToTranslate> {
        serde_json::from_reader(opened_file).expect("cached file couldn't be deserialize")
    }

    const GEMINI_RAW_RESPONSE_FILE_NAME: &str = "gemini_raw_response.txt";
    const RES_CACHE_FILE_NAME: &str = "tranai_cache.json";

    impl TranAI {
        pub fn new() -> TranAI {
            TranAI
        }

        async fn translate_book(
            book: &mut Book,
            mut url: Url,
            mut images: Vec<PathBuf>,
            pro: &bool,
        ) -> anyhow::Result<()> {
            let sys_prompt = fetch_url(url);
            let mut for_translation: Vec<ToTranslate> = vec![];
            for item in book.iter() {
                if let BookItem::Chapter(ref ch) = *item {
                    let newtt = ToTranslate {
                        file_path: ch.clone().path.unwrap(),
                        content: ch.clone().content,
                        chapter_title: ch.clone().name,
                    };
                    for_translation.push(newtt);
                }
            }

            let file = File::open(RES_CACHE_FILE_NAME);
            let mut cached_translations = if let Ok(opened_file) = file {
                let cached_translations: Vec<ToTranslate> = get_cached_translations(opened_file);
                let cached_paths: HashSet<PathBuf> = HashSet::from_iter(
                    cached_translations
                        .iter()
                        .clone()
                        .map(|el| el.file_path.clone()),
                );
                for_translation = for_translation
                    .into_iter()
                    .filter(|el| !cached_paths.contains(&el.file_path))
                    .collect();
                cached_translations
            } else {
                vec![]
            };

            let gemini: GeminiConf = GeminiConf {
                api_key: env::var("GEMINI_API_KEY")?,
                url: None,
            };
            use gemini_rust::prelude::*;

            let mut gemini_response = if *pro {
                Gemini::pro(gemini.api_key)
            } else {
                Gemini::new(gemini.api_key)
            }
            .expect("no connection to gemini")
            .generate_content()
            .with_system_prompt(sys_prompt)
            .with_response_mime_type("application/json")
            .with_response_schema(json!(
            {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to the translated chapter."
                        },
                        "content": {
                            "type": "string",
                            "description": "Content of the translated chapter."
                        },
                        "chapter_title": {
                            "type": "string",
                            "description": "Newly translated chapter."
                        },
                    },
                    "required": ["file_path", "content", "chapter_title"],
                }
            }));

            gemini_response.contents = Vec::new();
            let mut parts: Vec<Part> = vec![];
            parts.push(Part::Text {
                text: serde_json::to_string(&for_translation)?,
                thought: None,
                thought_signature: None,
            });
            gemini_response.contents.push(Content {
                parts: Some(parts),
                role: Some(Role::User),
            });

            let mut translated_response = {
                let gemini_response = gemini_response.execute().await;
                let res_to_str = gemini_response.unwrap().text();

                let mut file = File::create(GEMINI_RAW_RESPONSE_FILE_NAME)?;
                file.write_all(res_to_str.as_bytes())?;

                serde_json::from_str::<Vec<ToTranslate>>(&res_to_str)
                .unwrap_or_else(|er|{
                    eprintln!("Cannot parse response translation from gemini to internal data type. {er} Check {GEMINI_RAW_RESPONSE_FILE_NAME} for how the prompt looks and add it to the {RES_CACHE_FILE_NAME}. Make it correct json array.");
                    vec![]
                })
            };

            translated_response.append(&mut cached_translations);
            // Yes I know it's not optimal :)
            for each in translated_response.iter_mut() {
                book.for_each_chapter_mut(|chapter| {
                    if chapter.path.clone().unwrap() == each.file_path {
                        chapter.name = each.chapter_title.clone();
                        chapter.content = each.content.clone();
                    }
                });
            }

            Ok(())
        }
    }

    fn make_absolute(images: &mut Vec<PathBuf>, root: &PathBuf) {
        for p in images.iter_mut() {
            let mut new_path = PathBuf::new();
            new_path.push(root);
            new_path.push(&p);
            p.clear();
            p.push(new_path);
        }
    }

    impl Preprocessor for TranAI {
        fn name(&self) -> &str {
            TRAN_NAME
        }

        fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
            // In testing we want to tell the preprocessor to blow up by setting a
            // particular config value
            match (
                ctx.config
                    .get::<Url>(&format!("preprocessor.{TRAN_NAME}.custom_prompt")),
                ctx.config
                    .get::<Vec<PathBuf>>(&format!("preprocessor.{TRAN_NAME}.images")),
                ctx.config
                    .get::<Option<bool>>(&format!("preprocessor.{TRAN_NAME}.use_pro")),
            ) {
                (Ok(Some(url)), Ok(Some(mut images)), Ok(Some(mut pro))) => {
                    make_absolute(&mut images, &ctx.root);
                    let rt = Runtime::new().expect("Failed to crate tokio runtime");
                    rt.block_on(TranAI::translate_book(
                        &mut book,
                        url,
                        images,
                        pro.get_or_insert(false),
                    ))
                    .map_err(|e| mdbook_preprocessor::errors::Error::from(e));
                }
                _ => anyhow::bail!(
                    "Not all properties were specified in config, check example toml file."
                ),
            }

            // we *are* a no-op preprocessor after all
            Ok(book)
        }

        fn supports_renderer(&self, renderer: &str) -> Result<bool> {
            Ok(renderer != "not-supported")
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn nop_preprocessor_run() {
            let input_json = r##"[
                {
                    "root": "/path/to/book",
                    "config": {
                        "book": {
                            "authors": ["AUTHOR"],
                            "language": "en",
                            "src": "src",
                            "title": "TITLE"
                        },
                        "preprocessor": {
                            "nop": {}
                        }
                    },
                    "renderer": "html",
                    "mdbook_version": "0.4.21"
                },
                {
                    "items": [
                        {
                            "Chapter": {
                                "name": "Chapter 1",
                                "content": "# Chapter 1\n",
                                "number": [1],
                                "sub_items": [],
                                "path": "chapter_1.md",
                                "source_path": "chapter_1.md",
                                "parent_names": []
                            }
                        }
                    ]
                }
            ]"##;
            let input_json = input_json.as_bytes();

            let (ctx, book) = mdbook_preprocessor::parse_input(input_json).unwrap();
            let expected_book = book.clone();
            let result = TranAI::new().run(&ctx, book);
            assert!(result.is_ok());

            // The nop-preprocessor should not have made any changes to the book content.
            let actual_book = result.unwrap();
            assert_eq!(actual_book, expected_book);
        }
    }
}
