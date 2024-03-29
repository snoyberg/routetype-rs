# routetype

[![Rust](https://github.com/snoyberg/routetype-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/snoyberg/routetype-rs/actions/workflows/rust.yml)

This repository is a work in progress, experimental exploration of strongly typed routing in Rust. It follows my previous work with [Yesod](https://www.yesodweb.com/) in Haskell.

## What's a strongly typed route?

With strongly typed routes, you have a single `enum` (or, in Haskell, ADT) that represents all potentially valid URLs in your web application. This type has parse and render functions, typically automatically generated by metaprogramming to avoid boilerplate errors. Entry to your application starts by calling that parse function to generate a value of this type. Instead of using string interpolation to generate links within your application, you use the render function.

## Why would I want strongly typed routes?

There are a few advantages:

* If you change the structure of your routes, generated code automatically updates.
* More strongly, if you modify the parameters to your routes, existing code will fail to compile, and force you to update code appropriately.
    * In my opinion, this is the single greatest strength of a strongly typed codebase: common mistakes are converted into compile time errors, and the compiler can tell you exactly what you need to fix.
* It can be far less tedious to generate URLs this way.
* There's one central data type you can reference to see all of the different parts of your application.

## What does this project consist of?

This is a work in progress, but for now it includes:

* `routetype`
    * Helper functions for parsing paths and query strings, properly supporting URL decoding and corner cases like "query string keys without values," e.g. `?foo&bar&baz`
    * A `Route` trait for strongly typed routes
* `routetype-derive`: A derive macro for the `Route` type. This is exported automatically from `routetype`.
* `routetype-warp`: A few `Filter`s for parsing and dispatching impls of `Route`

## What's coming next?

Possibly nothing. Possibly:

* Building out additional support for Warp
* Adding support for other frameworks like actix
* Adding a lower level Hyper-specific binding
    * This likely would come with other helper functions to build out a microframework for simple apps
* Actually releasing what's already here to crates.io!
* Support multipath pieces, e.g. a trailing `Vec` of `String`s
* Support embedding other routes within this route, which may simply rely on the multipath concept

If this is interesting, and you'd like to be a part, jump in! No guarantees on anything, but issues, PRs, and direct messages anywhere about your interest in the project are more likely to push me into turning this into something real.
