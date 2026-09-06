# vanadis

Build your CLI tool configs from one palette. Templates in, themed configs out.

> **Status: not usable yet.** The design is being worked out in
> [issues](https://github.com/torabit/vanadis/issues). See the
> [v0.1 milestone](https://github.com/torabit/vanadis/milestone/1) for what "usable" means.

## The problem

Colour lives in every tool's own config file. Changing one shade means editing ten files,
and they drift apart. Switching between a light and a dark scheme means editing them all
again.

## The idea

One palette. Templates alongside your existing configs. One command renders them and
reloads the tools that can reload.

```
vanadis apply nord
vanadis apply --variant dark
vanadis check                   # do the generated files still match their templates?
vanadis apply nord --diff       # what would change?
vanadis get role.bg             # for a tool that would rather ask than read a file
```

## What this is not

A theme distribution system. If you want to browse hundreds of ready-made themes for tools
other people already support, use [tinty](https://github.com/tinted-theming/tinty) — that
is what it is for and it does it well.

vanadis is for the case tinty does not cover: you have written your own configs, for tools
that may have no template repository at all, in a palette that may not fit base16's sixteen
slots. It renders your templates from your palette. Nothing to conform to.

| | renders templates | vocabulary | needs |
| --- | --- | --- | --- |
| [tinty](https://github.com/tinted-theming/tinty) | no, copies pre-built files | base16 / base24 / tinted8 | template repos |
| [flavours](https://github.com/Misterio77/flavours) | yes | base16's 16 slots | — |
| [Stylix](https://github.com/danth/stylix) | yes | base16 | Nix |
| vanadis | yes | arbitrary | — |

It can still import from the [tinted-theming schemes](https://github.com/tinted-theming/schemes)
collection, so the existing theme library is not lost.

## Licence

MIT OR Apache-2.0.
