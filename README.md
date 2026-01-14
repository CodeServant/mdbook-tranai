This is preprocessor for gemini ai automatic translation.
This is so far very experimental and dirty (the code needs thorough quality check).



## Test
Install this extension with `cargo install --path .`. And then install it  within mdbook add to `book.toml`
```toml
[preprocessor.tranai]
images = ["some.txt"] # list of images that should be translated, relative to this file
custom_prompt = "file://../hello.txt"
```
So far there are files that appear when i want to read a message.