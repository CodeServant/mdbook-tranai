This is preprocessor for gemini AI automatic translation.
This is so far very experimental and dirty (the code needs thorough quality checks).



## Test
Install this extension with `cargo install --path .`. And then install it  within mdbook add to `book.toml`
```toml
# there is a need to configure GEMINI_API_KEY env variable
[preprocessor.tranai]
images = ["some.txt"] # not implemented  # list of images that should be translated, relative to this file
custom_prompt = "file://absolute/path/hello.txt" # it must be absolute path can be https
```
So far there are files that appear when i want to read a message.