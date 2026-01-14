//! A basic example of a preprocessor that does nothing.

use crate::nop_lib::TranAI;
use anyhow::anyhow;
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
    use std::{
        char::ToUppercase,
        env::{self, consts},
        fs::File,
        io::Write,
        path::{Path, PathBuf},
        vec,
    };

    use clap::builder::UnknownArgumentValueParser;
    use gemini_rust::{Part, client::GeminiClient};
    use mdbook_preprocessor::book::BookItem;
    use serde::{Deserialize, Serialize};
    use serde_json::{json, to_string_pretty};
    use tokio::runtime::Runtime;
    use url::Url;

    use super::*;

    /// A no-op preprocessor.
    pub struct TranAI;
    struct GeminiConf {
        api_key: String,
        url: Option<Url>,
    }
    fn fetch_url(url: Url) -> String {
        return reqwest::blocking::get(url.as_str())
            .expect("cannot download custom prompt")
            .text()
            .expect("response from custom prompt invalid");
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct ToTranslate<'a> {
        file_path: &'a Path,
        content: &'a str,
    }

    impl <'a>fmt::Display for ToTranslate<'a> {
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
            mut book: &Book,
            mut url: Url,
            mut images: Vec<PathBuf>,
        ) -> anyhow::Result<()> {
            let sys_prompt = "Zamień ten text na język polski. Zwróć podobny JSON jak ten wejściowy. Całego jsona w jednej linijce."; //fetch_url(url);
            let mut wynik: Vec<ToTranslate> = vec![];
            for item in book.iter() {
                if let BookItem::Chapter(ref ch) = *item {
                    let newtt = ToTranslate {
                        file_path: ch.path.as_ref().unwrap(),
                        content: &ch.content,
                    };
                    wynik.push(newtt);
                }
            }

            let mut file = File::create("wynik.txt")?;
            file.write_all(to_string_pretty(&sys_prompt).unwrap().as_bytes())?;

            let gemini: GeminiConf = GeminiConf {
                api_key: env::var("GEMINI_API_KEY")?,
                url: None,
            };
            use gemini_rust::prelude::*;

            let mut gemini_response = Gemini::new(gemini.api_key)
                .expect("no connection to gemini")
                .generate_content()
                .with_system_prompt(sys_prompt)
                .with_response_mime_type("application/json");
                //.with_response_schema(json!([{"file_path": "{file_path}","content": "{content}"}]));

            gemini_response.contents = Vec::new();
            let mut parts: Vec<Part> = vec![];
            parts.push(Part::Text {
                text: serde_json::to_string(&wynik)?,
                thought: None,
                thought_signature: None,
            });
            let mut file = File::create("parsed_json_req.txt")?;
            file.write_all(serde_json::to_string(&parts)?.as_bytes())?;

            gemini_response.contents.push(Content {
                parts: Some(parts),
                role: Some(Role::User),
            });
            
            let mut file = File::create("gemini_req.txt")?;
            
            //file.write_all(serde_json::to_string(&gemini_response.build())?.as_bytes())?;

            //let gemini_response = gemini_response.execute().await;

            let mut file = File::create("gemini_resp.txt")?;
            //let res_to_str = gemini_response.unwrap().text();
            //let translated = serde_json::from_str::<Vec<ToTranslate>>(&res_to_str).expect("couldn't translate response from gemini to internal type");

            //file.write_all(serde_json::to_string(&res_to_str)?.as_bytes())?;

            todo!("Gemini translation for the whole book");
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
                    let mut file = File::create("flag.txt")?;
                    file.write_all("after translation it passes".as_bytes())?;
                }
                _ => anyhow::bail!(
                    "Not all properties in config were specified, check example toml file."
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
