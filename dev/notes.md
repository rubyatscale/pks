# TODO
## Features
- `packs init | create | move`
- CLI could have `-i` interactive mode (like `use_packs`, also see https://github.com/mikaelmello/inquire)
- Privacy violation inversion?

## Performance
Although `pks` is intended to be fast, there are ways it can be made a lot faster!
- Conditional cache usage. For example, implemented as an LSP, pks could always use cache and only bust specific caches (asychronously) when certain events (e.g. file changes) are received.
- By using modified time, we can avoid opening the entire file and parsing it and calculating the md5 hash. It's possible this would not be a meaningful performance improvement.

### Improved use of references (less cloning)
As I'm new to Rust, I don't take advantage of a lot of features in Rust that would improve the performance, such as making sure I minimize the use of deep clones and use references.

# Milestones
- [x] Generate `packwerk` compatible cache with `packs generate_cache`
- [x] Parse ERB files
- [x] `packs update`, which can be used to update `package_todo.yml`
- [x] `packs check`, which can be used as a drop-in replacement to the VSCode

# Profiling
I've been using https://github.com/flamegraph-rs/flamegraph to generate flamegraphs to improve performance.

Install with:
```
cargo install flamegraph
```

I have aliased this command which generates something like a "left-shifted" flamegraph, to show where most of the time is spent:
```
alias profile_packs='CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph --profile=release --reverse --min-width=0.5 -- --project-root="../your_app" check'
```
For more, see: https://nnethercote.github.io/perf-book/profiling.html

# Local Development
## Running the CLI in release mode against a target app
```
time cargo run --profile=release -- --debug --project-root=../your_app check
```

# Packwerk Implementation Considerations
- See `EXPERIMENTAL_PARSER_USAGE.md` for more info
- Packwerk considers a definition to be a reference. I explored removing this in this branch: https://github.com/alexevanczuk/packs/pull/44
  - This results in a diff in violations, because if a class opens up a module defined by another class, its considered to be a reference to that other class.
  - I think this is actually a bug in packwerk, since a definition is not really a reference. Even though monkey patching / opening up other moduels is not great, we should surface that information through a different mechanism (such as allowing packs to have a monkey patches violation)

# Abandoned Performance Improvement Attempts
- In https://github.com/alexevanczuk/packs/pull/37, I looked into getting the constants *as* we are walking the directory. However, I found that this was hardly much more performant than the current implementation, and it was much more complex. I abandoned this approach in favor of caching the resolver and other performance improvements.
- We could consider caching the RESOLVED references in a file, which would allow us to potentially skip generating the constant resolver and resolving all of the unresolved constants. This makes cache invalidation more complex though, and the flamegraph shows that most of the time is spent opening files, not resolving constants. Furthermore, the experimental constant resolver resolves constant much more quickly.
- In https://github.com/alexevanczuk/packs/pull/98, I looked into having a single file as the cache rather than one cache file per code file. This turned out to be a *lot* slower, and I think the reason is that serialization and deserialization happens does not happen in parallel with one large file, where it does happen with lots of tiny files.
- In https://github.com/alexevanczuk/packs/pull/99, I began (very initial stages) of integrating with SQLite to hopefully provide a faster cache. It's not clear to me if this would actually provide much of a performance improvement. It might still be worth exploring, but for now abandoning it since it introduces a lot of complexity.

# Modular Architecture
Today, `packwerk` has a modular architecture allowing folks to add new checkers, validators, etc.
Eventually, I'd like to port this idea over to `packs`.
We might consider how we can have specific checkers/validators be responsible for their own portion of the deserialized properties in `package.yml` files.
