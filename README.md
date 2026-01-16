A [preprocessor](https://rust-lang.github.io/mdBook/format/configuration/preprocessors.html) for [*mdBook*](https://rust-lang.github.io/mdBook/) for automatic translations using AI (currently [*Gemini*](https://gemini.google.com)).


This is prof of concept (the code needs thorough quality checks and testing), but you can use it, it works (if *Gemini* doesn't ditch you). It was used to translate rust book into [polish](https://www.macia.pl/docs/rust-book) language.

## How to use

### Requirements
- *mdBook* [installed](https://rust-lang.github.io/mdBook/guide/installation.html#build-from-source-using-rust)
- Install this extension with [*cargo*](https://rustup.rs/) `cargo install --git https://github.com/CodeServant/mdbook-tranai.git`.
- [*Gemini* API](https://ai.google.dev/gemini-api/docs/api-key) key in your `GEMINI_API_KEY` [environmental variable](https://en.wikipedia.org/wiki/Environment_variable).

When you want to translate complete mdBook, **add** this **snippet** to **the bottom** of `book.toml` file. Tweak it a bit.
```toml
# configure GEMINI_API_KEY env variable
[preprocessor.tranai]
images = ["some.txt"] # not implemented  # list of images that should be translated, relative to this file
custom_prompt = "https://raw.githubusercontent.com/CodeServant/mdbook-tranai/refs/heads/experimental/langs/pl.md" # file with prompt to gemini, it must be absolute path can be https, file (md, txt), this is the file which determine the out language (best to write in your destination language)
use_pro = false # turn on the Gemini Pro for better results (Flash when false)
```

Now when you run `mdbook build --open` your book will open (after a while) in your preferred language.

## Caveats
- Sometimes Gemini end the response suddenly. When that happens this preprocessor saves fetched response to a file (you will be prompted about it in the error message). Then paste this response array without broken element to the cache json file (name in err msg) in the main dir. This cache is then used to filter out messages that were translated. If cache consists all messages, then there is no connection to Gemini. **Check the last message output**. Sometimes *Gemini* can change the last message even when it correctly create JSON.
- **Don't** use `mdBook watch/serve` (every refresh will eat your tokens). And when there is cached json using this features makes no sense, because on the output you will have only translated document from cache (cache is applied by file names for chapters).

## Keep in mind
- This preprocessor must be executed last (order is alphabetical). Use [mdBook ordering](https://rust-lang.github.io/mdBook/format/configuration/preprocessors.html?highlight=order#require-a-certain-order).
- It's ***Google Gemini*** (payed, external service). Every compile sends data to them and **eat your tokens**.
- You can have link to **external source** of custom prompt (so check if you **trust the prompting person**).
- This program won't be very generous with messages when error occurs.


## Ideas for features
- Translating images with [*Nano Banana*](https://gemini.google/pl/overview/image-generation) model.
- Create central prompts directory or maybe compile to source code.