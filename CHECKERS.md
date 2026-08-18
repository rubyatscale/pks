# Checkers
## Privacy Checker
The privacy checker extension was originally extracted from [packwerk](https://github.com/Shopify/packwerk).

A package's privacy boundary is violated when there is a reference to the package's private constants from a source outside the package.

To enforce privacy for your package, set `enforce_privacy` to `true` or `strict` on your pack:

```yaml
# components/merchandising/package.yml
enforce_privacy: true
```

Setting `enforce_privacy` to `true` will make all references to private constants in your package a violation.

Setting `enforce_privacy` to `strict` will forbid *new* references to private constants in your package. **Violations already recorded in the referencing package's `package_todo.yml` are tolerated**, so strict mode stops the list growing rather than requiring it to be empty.

### Adopting strict mode on a package that already has violations

**Record the existing violations first, then flip to `strict`.** The order matters, because tolerance only ever matches entries that are *already* in a `package_todo.yml`, and `update` will not create them once the package is strict:

```sh
# 1. while the package is still `enforce_privacy: true`
pks update

# 2. commit the package_todo.yml files this wrote

# 3. now set enforce_privacy: strict
```

Flipping to `strict` first leaves you stuck: `check` fails on the existing references, and `pks update` will not record them, so the only ways out are fixing every reference, hand-writing the todo entries, or reverting to `true`. pks matches packwerk here.

To see everything the todo files are currently grandfathering, run `pks check --ignore-recorded-violations`.

Once the package is strict, `pks update` will not add new entries for it: an unrecorded strict violation is never written to a `package_todo.yml`, so it keeps failing until the reference is dealt with. Note that this is a guarantee about `update`, not about the file. A hand-added entry does silence strict mode, and `update` preserves it rather than dropping it, so the boundary is only as strong as your review of `package_todo.yml` diffs.

### Using public folders
You may enforce privacy either way mentioned above and still expose a public API for your package by placing constants in the public folder, which by default is `app/public`. The constants in the public folder will be made available for use by the rest of the application.

### Defining your own public folder

You may prefer to override the default public folder, you can do so on a per-package basis by defining a `public_path`.

Example:

```yaml
public_path: my/custom/path/
```

### Defining public constants through sigil

> [!WARNING]
> This way of defining the public API of a package should be considered WIP. It is not supported by all tooling in the RubyAtScale ecosystem, as @alexevanczuk pointed out in a [comment on the PR](https://github.com/rubyatscale/packwerk-extensions/pull/35#discussion_r1334331797):
>
> There are a couple of other places that will require changes related to this sigil. Namely, everything that is coupled to the public folder implementation of privacy.
>
> In the rubyatscale org:
>
> * pack_stats, example https://github.com/rubyatscale/pack_stats/blob/main/lib/pack_stats/private/metrics/public_usage.rb. (IMO though we can just remove this metric – it has never been useful)
> * Other places that mention public_path or app/public.
>   * Org wide search for app/public link
>   * Org wide search for public_path link
>   * packs (the Rust port of packwerk – I could take this one over unless someone is interested in implementing whatever we come up with there



You may make individual files public within a private package by usage of a comment within the first 5 lines of the `.rb` file containing `pack_public: true`.

Example:

```ruby
# pack_public: true
module Foo
  class Update
  end
end
```
Now `Foo::Update` is considered public even though the `foo` package might be set to `enforce_privacy: (true || strict)`.

It's important to note that when combining `public_api: true` with the declaration of `private_constants`,
`packwerk validate` will raise an exception if both are used for the same constant. This must be resolved by removing
the sigil from the `.rb` file or removing the constant from the list of `private_constants`.

If you are using rubocop, it may be configured in such a way that there must be an empty line after the magic keywords at the top of the file. Currently, this extension is not modifying rubocop in any way so it does not recognize `pack_public: true` as a valid magic keyword option. That means placing it at the end of the magic keywords will throw a rubocop exception. However, you can place it first in the list to avoid an exception in rubocop.
```
-----
# typed: ignore
# frozen_string_literal: true
# pack_public: true

class Foo
...
end => Layout/EmptyLineAfterMagicComment: Add an empty line after magic comments.

------
# typed: ignore
# frozen_string_literal: true

# pack_public: true

class Foo
...
end => Less than ideal. This won't raise an issue in rubocop, however, only the first 5 lines are scanned for the magic comment of pack_public so there is risk at it being missed. It also is requiring extra empty lines in the group of magic comments.

-----
# pack_public: true
# typed: ignore
# frozen_string_literal: true

class Foo
...
end => Ideal solution. No exceptions from rubocop and very low risk of the magic comment being out of range since
```

### Using specific private constants
Sometimes it is desirable to only enforce privacy on a subset of constants in a package. You can do so by defining a `private_constants` list in your package.yml. Note that `enforce_privacy` must be set to `true` or `'strict'` for this to work.

### Ignore strict mode for violations coming from specific path patterns
You do not need this to adopt `'strict'` mode on a package that already has violations you will deal with later: record them first and they are tolerated, as described above. Reach for a path exemption when you want to exempt a **path** rather than a recorded list.

Use [`enforcement_globs_ignore`](#enforcement-globs-ignore) with `enforcements: [privacy]`:

```yaml
enforce_privacy: strict

enforcement_globs_ignore:
- enforcements:
  - privacy
  ignores:
  - engines/another_engine/test/**
  reason: test files reach into engine internals
```

In this example, privacy violations on constants of your engine referenced from anywhere under `engines/another_engine/test/` will not fail pks checks.

Note the trailing `**` rather than `**/*`. These are gitignore-style globs, so `**` matches the whole subtree including files directly inside `test/`, whereas `**/*` requires at least one intervening directory and would silently skip `test/a_test.rb`. A pattern that matches nothing looks identical to no exemption at all, so check a new pattern against a file you expect it to cover.

> **Note:** packwerk spells this `strict_privacy_ignored_patterns`. **pks does not implement that key**, and because `Pack` collects unknown keys via `#[serde(flatten)]` it is accepted silently and has no effect, which leaves the pack unguarded. Use `enforcement_globs_ignore` instead.

The two mechanisms differ in what they grandfather, so they are not interchangeable. A `package_todo.yml` entry covers one constant referenced from one file, for one violation type, so a reference to a *different* constant from that same file still fails. A path exemption covers the path outright, so anything those files reference later is ignored too. Prefer the todo file unless you genuinely want the whole path exempt.

### Package Privacy violation
Packwerk thinks something is a privacy violation if you're referencing a constant, class, or module defined in the private implementation (i.e. not the public folder) of another package. We care about these because we want to make sure we only use parts of a package that have been exposed as public API.

#### Interpreting Privacy violation

> /Users/JaneDoe/src/github.com/sample-project/user/app/controllers/labels_controller.rb:170:30
> Privacy violation: '::Billing::CarrierInvoiceTransaction' is private to 'billing' but referenced from 'user'.
> Is there a public entrypoint in 'billing/app/public/' that you can use instead?
>
> Inference details: 'Billing::CarrierInvoiceTransaction' refers to ::Billing::CarrierInvoiceTransaction which seems to be defined in billing/app/models/billing/carrier_invoice_transaction.rb.

There has been a privacy violation of the package `billing` in the package `user`, through the use of the constant `Billing::CarrierInvoiceTransaction` in the file `user/app/controllers/labels_controller.rb`.

#### Suggestions
You may be accessing the implementation of a piece of functionality that is supposed to be accessed through a public interface on the package. Try to use the public interface instead. A package’s public interface should be defined in its `app/public` folder and documented.

The functionality you’re looking for may not be intended to be reused across packages at all. If there is no public interface for it but you have a good reason to use it from outside of its package, find the people responsible for the package and discuss a solution with them.

## Visibility Checker
The visibility checker can be used to allow a package to be a private implementation detail of other packages.

To enforce visibility for your package, set `enforce_visibility` to `true` on your pack and specify `visible_to` for other packages that can use your package.

```yaml
# components/merchandising/package.yml
enforce_visibility: true
visible_to:
  - components/other_package
```

## Folder-Privacy Checker
The folder privacy checker can be used to allow a package to be private to their sibling packs and parent packs and will create todos if used by any other package.

To enforce folder privacy for your package, set `enforce_folder_privacy` to `true` on your pack.

```yaml
# components/merchandising/package.yml
enforce_folder_privacy: true
```

Here is an example of paths and whether their use of `packs/b/packs/e` is OK or not, assuming that protects itself via `enforce_folder_privacy`

```
.                         OK (parent of parent)
packs/a                   VIOLATION
packs/b                   OK (parent)
packs/b/packs/d           OK (sibling)
packs/b/packs/e           ENFORCE_NESTED_VISIBILITY: TRUE
packs/b/packs/e/packs/f   VIOLATION
packs/b/packs/e/packs/g   VIOLATION
packs/b/packs/h           OK (sibling)
packs/c                   VIOLATION
```

## Layer Checker
The layer checker can be used to enforce constraints on what can depend on what.

To enforce layers for your package, first define the `layers` in `packwerk.yml`, for example:
```
layers:
  - package
  - utility
```

Then, turn on the checker in your package:
```yaml
# components/merchandising/package.yml
enforce_layers: true
layer: utility
```

Now this pack can only depend on other utility packages.

# Enforcement Globs Ignore
`enforcement_globs_ignore` can be used to specify gitignore-style rules for not enforcing violations.

### Examples

```yml
# packs/product_services/serv1/foo/package.yml
enforce_privacy: true
enforce_visibility: true

enforcement_globs_ignore:
- enforcements:
  - privacy
  - visibility
  ignores:
  - "**/*"
  # Enforce incoming privacy and visibility violation references _only_ in `pks/product_services/serv1/**/*`
  - "!packs/product_services/serv1/**/*"
  reason: "It was decided only to fix incoming violations from serv1. See ticket #232"
```

```yml
# packs/pack2/package.yml
enforce_dependencies: true
dependencies:
# not required because of the below enforcement_globs_ignore
# - packs/pack1 
# required because of the enforcement_globs_ignore exception line 
  - packs/pack3 

enforcement_globs_ignore:
- enforcements:
  - dependency
  ignores:
  - "**/*"
  # Enforce outgoing dependency violation references _only_ to `pks/pack3/**/*`
  - "!packs/pack3/**/*"
  reason: "The other dependency violations are fine as those packs will be absorbed into this one."
```

