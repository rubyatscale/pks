# Violation Types
`pks`is a rust replacement for [packwerk](https://github.com/Shopify/packwerk) and [packwerk-extensions](https://github.com/rubyatscale/packwerk-extensions).

`pks` is used to enforce architecture rules for ruby monolith applications. Monoliths are split up into "packs", which are groupings of related code.
Currently, `pks` ships the following checkers to help improve the boundaries between packages. These checkers are:

- A privacy checker that ensures other packages are using your package's public API
- A visibility checker that allows packages to be private except to an explicit group of other packages.
- A folder_privacy checker that allows packages to their sibling packs and parent pack (to be used in an application that uses folder packs)
- A layer (formerly architecture) checker that allows packages to specify their "layer" and requires that each layer only communicate with layers below it.


## Dependency
## Privacy
## Layers
## Folder Privacy
## Visibility