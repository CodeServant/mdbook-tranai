A [preprocessor](https://rust-lang.github.io/mdBook/format/configuration/preprocessors.html) for [*mdBook*](https://rust-lang.github.io/mdBook/) for automatic translations using AI (currently [*Gemini*](https://gemini.google.com)).


This is prof of concept (the code needs thorough quality checks and testing), but you can use it, it works (if *Gemini* doesn't ditch you).

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
custom_prompt = "https://raw.githubusercontent.com/CodeServant/mdbook-tranai/refs/heads/experimental/langs/pl.md" # txt file with prompt to gemini, it must be absolute path can be https, file (md, txt), this is the file which determine the out language (best to write in your destination language)
# use_pro = true # optionally turn on the Gemini Pro for better results
```

Now when you run `mdbook build --open` your book will open (after a while) in your preferred language.

## Caveats
- Note that gemini can sometimes **produce results** that are **not respected** by this program (then you have to retry). Hope that *Gemini Pro* do better.
- **Don't** use `mdBook watch/serve` (every refresh will eat your tokens)

## Keep in mind
- It's ***Google Gemini*** (payed, external service). Every compile sends data to them and **eat your tokens**.
- You can have link to **external source** of custom prompt (so check if you **trust the prompting person**).
- This program won't be very generous with messages when error occurs.


## Ideas for features
- Translating images with [*Nano Banana*](https://gemini.google/pl/overview/image-generation) model.