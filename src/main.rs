//! A basic example of a preprocessor that does nothing.

use crate::nop_lib::TranAI;
use clap::{Arg, ArgMatches, Command};
use mdbook_preprocessor::book::Book;
use mdbook_preprocessor::errors::Result;
use mdbook_preprocessor::{Preprocessor, PreprocessorContext};
use semver::{Version, VersionReq};
use std::io;
use std::process;

fn make_app() -> Command {
    Command::new("tranai")
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
    use core::fmt;
    use std::{env, fs::File, io::Read, path::PathBuf, vec};

    use gemini_rust::Part;
    use mdbook_preprocessor::book::BookItem;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
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
                let mut opened =
                    File::open(url.path()).unwrap_or_else(|e| panic!("Could not open a file: {e}"));
                let mut buf = String::new();
                opened.read_to_string(&mut buf);
                return buf;
            }
            "https" => {
                let res = reqwest::blocking::get(url);
                res.unwrap_or_else(|e| panic!("Could not download text: {e}"))
                    .text()
                    .unwrap_or_else(|e| panic!("Could not decode text after download: {e}"))
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

    impl<'a> fmt::Display for ToTranslate {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "{{file_path: {:#?}, content: {:#?}}}",
                self.file_path, self.content
            )
        }
    }

    impl TranAI {
        pub fn new() -> TranAI {
            TranAI
        }

        async fn translate_book(
            book: &mut Book,
            mut url: Url,
            mut images: Vec<PathBuf>,
        ) -> anyhow::Result<()> {
            let sys_prompt = fetch_url(url);
            let mut wynik: Vec<ToTranslate> = vec![];
            for item in book.iter() {
                if let BookItem::Chapter(ref ch) = *item {
                    let newtt = ToTranslate {
                        file_path: ch.clone().path.unwrap(),
                        content: ch.clone().content,
                        chapter_title: ch.clone().name,
                    };
                    wynik.push(newtt);
                }
            }

            let gemini: GeminiConf = GeminiConf {
                api_key: env::var("GEMINI_API_KEY")?,
                url: None,
            };
            use gemini_rust::prelude::*;

            let mut gemini_response = Gemini::new(gemini.api_key)
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
                            }
                        }
                    }
                }));

            gemini_response.contents = Vec::new();
            let mut parts: Vec<Part> = vec![];
            parts.push(Part::Text {
                text: serde_json::to_string(&wynik)?,
                thought: None,
                thought_signature: None,
            });
            gemini_response.contents.push(Content {
                parts: Some(parts),
                role: Some(Role::User),
            });

            let gemini_response = gemini_response.execute().await;
            let res_to_str = gemini_response.unwrap().text();

            let mut translated_response = serde_json::from_str::<Vec<ToTranslate>>(&res_to_str)
                .expect("cannot parse response translation from gemini to internal data type");

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

    const TRAN_NAME: &str = "tranai";
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
            ) {
                (Ok(Some(url)), Ok(Some(mut images))) => {
                    make_absolute(&mut images, &ctx.root);
                    let rt = Runtime::new().expect("Failed to crate tokio runtime");
                    rt.block_on(TranAI::translate_book(&mut book, url, images))
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
