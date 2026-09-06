# Remote schemes

This document decides where vanadis gets the tinted-theming scheme collection, how it is
cached, how `vanadis search` finds a scheme in it, and what `vanadis import` writes when it
turns one into a theme. It builds on [docs/config.md](config.md), which decides every other
path vanadis uses.

The collection is the reason a new user has anything to apply on the first day. The mapping
from a scheme's palette onto a theme's tokens is not decided here:
[docs/core-vocabulary.md](core-vocabulary.md#base16-onto-the-core) decides it for base16 and
base24, and the module documentation of `src/scheme/convert/tinted8.rs` for tinted8. This
document stops at having the upstream files on disk, finding one, and putting the result
somewhere `apply` can reach it.

```
$ vanadis remote update
fetched 538 schemes: 338 base16, 196 base24, 4 tinted8

$ vanadis search nord
  base16/nord        dark   Nord        arcticicestudio
  base16/nord-light  light  Nord Light  threddast, based on fuxialexander's doom-nord-light-theme (Doom Emacs)
  tinted8/nord       dark   Nord        Tinted Theming (https://github.com/tinted-theming)

$ vanadis import nord
imported base16/nord as nord
  ~/.config/vanadis/themes/nord.toml  dark  Nord  arcticicestudio
```

## The source

```
https://github.com/tinted-theming/schemes/archive/refs/heads/spec-0.11.tar.gz
```

The URL is a constant in the binary. It is not configurable and there is no flag to override
it. A second source would need a rule for what happens when both hold `nord`, and there is no
second source: the collection is the one place base16, base24 and tinted8 schemes are
published together. Making it configurable is a change to make when a second one appears, not
before.

`spec-0.11` is the repository's default branch and the collection carries no tags, so a
branch is the only ref available to pin. When upstream moves to a new spec branch the
constant moves with it, in a release.

**Not the GitHub API.** Unauthenticated it allows 60 requests an hour, and listing a
directory of 538 files costs more than one request. The archive endpoint is one request for
the whole collection and is not part of that budget.

**Not git.** Cloning would carry the collection's history, need git on the machine or a git
library in the binary, and buy nothing: vanadis never reads a revision other than the tip.

## The cache

```
~/.cache/vanadis/
└── schemes/
    ├── LICENSE
    ├── base16/
    │   ├── nord.yaml
    │   └── ...
    ├── base24/
    └── tinted8/
```

The directory is `$XDG_CACHE_HOME/vanadis`, falling back to `~/.cache/vanadis` when
`XDG_CACHE_HOME` is unset. It follows `$XDG_CACHE_HOME` rather than `$VANADIS_CONFIG`, which
[docs/config.md](config.md#layout) lets replace the config directory, because this is
downloaded data and not something a user writes or backs up. Deleting the whole directory
costs one `remote update` and nothing else.

The files are the upstream files, byte for byte. Nothing is rewritten, normalised or
converted on the way in. A converter reads the same bytes a person reading the collection on
GitHub sees, so a disagreement between the two is a converter bug and not a cache artifact.

## What is extracted

The archive unpacks under a single `schemes-spec-0.11/` directory and holds a CI workflow, a
lint config, scripts and two README files besides the schemes. Only entries matching these
shapes are written, and the leading directory is dropped:

```
<root>/{base16,base24,tinted8}/<name>.yaml
<root>/{base16,base24,tinted8}/<name>.yml
<root>/LICENSE
```

`<name>` must be a plain filename: no path separator, and neither `.` nor `..`. The entry
must be a regular file, so symlinks and hard links are skipped. Everything else in the
archive is discarded.

Both extensions are matched because the collection uses both. `base16/cyberpunk.yml` is the
one file spelled `.yml`; every other scheme is `.yaml`. Matching only `.yaml` would drop a
scheme and say nothing.

This is a whitelist and not a sanitising pass. An entry naming `../../.ssh/authorized_keys`
matches none of the shapes above, so there is no path to sanitise and no traversal to
defend against: nothing outside the three scheme directories is ever a write target. The same
holds for an absolute path and for a deeper nesting.

`LICENSE` is kept because the collection is MIT and the licence text is the attribution.
Extracting the schemes without it would ship the collection's content and drop its terms.

A run writes into `schemes.incoming` beside the cache, then removes `schemes` and renames.
An interrupted download or a malformed archive leaves the previous cache in place, so
`search` never reads a half-written directory.

## Reading a scheme

`search` needs five fields per scheme: the system, the identifier, the display name, the
author and the variant. The identifier is the filename minus its extension, as in
`themes/`. The rest come out of the file, and the file's shape depends on the system.

The system comes from the directory the file is in, not from the `system` field. The
directory is what the extraction whitelist already matched on, so the field is a second copy
of a decision that is made before the file is opened. Reading it would only add a way for the
two to disagree, and a rule for which one wins.

base16 and base24 put everything else at the top level. All 338 base16 and 196 base24 files
carry `name`, `author` and `variant`.

```yaml
system: "base16"
name: "Nord"
author: "arcticicestudio"
variant: "dark"
```

tinted8 nests everything except `variant` under `scheme`, and its name is not one field.
Three of the four files spell the name as `family` plus `style`; `nord.yaml` spells it as
`name`. Both are read: `name` when the file has one, otherwise `family` and `style` joined by
a space.

```yaml
scheme:
  system: "tinted8"
  author: "Tinted Theming (https://github.com/tinted-theming)"
  family: "Catppuccin"
  style: "Latte"
variant: "light"
```

`theme-author` is not read. It credits whoever designed the colours upstream of the
collection, which is worth showing when a scheme is imported and is not what someone
searching by author is looking for.

**A YAML parser, not a line scanner.** Every field sits on its own line in every file,
so scanning for `name: ` would work on the collection as it stands today — except that one
base16 file writes `variant: dark` unquoted while every other file quotes it, which is
already two spellings of one value. The collection is not vanadis's to keep uniform, and a
scanner would fail silently and per-file when it changes.

A file that does not parse, or that is missing a field, costs the user that scheme and not
the command. It is skipped and reported, the same call
[docs/config.md](config.md#layout) makes for a `.toml` in `themes/` that does not load.

## search

```
vanadis search <QUERY>
```

A scheme matches when `QUERY`, compared without case, is a substring of any cell the line
prints: `system/id`, the variant, the name, or the author. The rule is that search matches
what search prints. It needs no explanation of which fields are indexed, and a result that
looks unrelated always has the reason visible on its own line.

Matching on `system/id` is what makes `search base16` list the base16 collection, and
matching on the author is what finds every scheme by one person.

Results are ordered by system, then by identifier. Systems sort `base16`, `base24`,
`tinted8`.

**`QUERY` is required.** Omitting it would print 538 lines, which is a different command with
different needs — paging, at least. Someone who wants all of them can ask for `base16`,
`dark` or any other cell they know is there.

The identifier is qualified because the bare one is not unique: `nord` is a base16 scheme and
a tinted8 scheme, and `gruvbox-dark` is in base16, base24 and tinted8. `base16/nord` is what
identifies a scheme, and it is printed in the form that can be handed to another command.

`search` reads the cache and never the network. It works on a machine that has been offline
since the last `remote update`.

A query nothing matches prints nothing and exits non-zero, which is what `grep` does and what
lets a script ask whether a scheme is there without parsing output. It is not an error and
prints no message: an empty result is an answer.

## import

```
vanadis import <SOURCE> [--force]
```

Converts one upstream scheme and writes it as a theme file under `themes/`, which
[docs/config.md](config.md#layout) decides the location of. After it, `apply` reaches the
theme by the name `import` printed and nothing else has to be done.

### What SOURCE names

Four shapes, tried in this order.

| shape | what it names |
| --- | --- |
| begins `http://` or `https://` | a scheme to fetch |
| `<system>/<id>`, `<system>` one of the three | one cached scheme |
| a single token segment | a cached scheme of that identifier, in any system |
| anything else | a file on disk |

A bare identifier takes the first system that holds it, searched `base16`, `base24`,
`tinted8`. `nord` is a base16 scheme and a tinted8 one, and `base16/nord` is how the loser is
said out loud. It is the same qualified form `search` prints, and this is the only command
other than `search` that reads it, because it is the only other command that names a scheme.

**A single segment always means the cache**, even when a file of that name sits in the
working directory. The other way round would make `import nord` depend on where it was run
from. A file is reached by writing a path to it, `./nord`, which is one character and is
already how a shell disambiguates a command from a file.

The cached lookup is by filename, not a scan. The identifier is the file stem, which
[Reading a scheme](#reading-a-scheme) decides, so six paths answer what reading 538 files
would. Both extensions are tried, for the reason [What is extracted](#what-is-extracted)
gives.

### The system of a file that is not in the cache

[Reading a scheme](#reading-a-scheme) decides that a cached scheme's system is the directory
it sits in and that the file's own `system` key is not read. That decision is about the
cache, where a directory exists. A path and a URL have none, so the file's own declaration is
the only evidence there is, and it is read: `system` at the top level, or `scheme.system`
when the file nests its header.

**There is no flag to override it.** A file that declares no system is not imported. The
alternative is guessing from the shape of the palette, which would read a base24 scheme
missing a slot as a base16 one and say nothing, and a wrong guess writes a theme whose bright
colours silently equal its normal ones.

### What the theme is called

The source's filename, minus its extension. For a cached scheme that is the identifier, for a
path it is the file's own name, and for a URL it is the last segment of the path. It has to
be a single token segment, because [docs/config.md](config.md#layout) makes the filename the
identifier; a source that gives anything else is refused rather than rewritten into one.

There is no flag to name it something else. Adding one is a change to make when somebody
wants two imports of the same scheme side by side; `--force` covers re-importing over one.

### A theme that is already there

`import` refuses, names the file, and exits non-zero. `--force` writes over it.

Refusing is the default because a theme file is a file a person edits. It is where a
per-target override's colours live and where a hand-written extra namespace goes, and
`docs/theme-format.md` treats those as the point of the format. Overwriting by default would
lose that work to a command whose failure mode is a typo in an identifier.

It is also not an edge case. `nord` is in base16 and in tinted8, and both want `nord.toml`.
Refusing turns that collision into a message; overwriting would turn it into whichever import
ran last.

The write is staged beside the destination and renamed over it, the way every other write in
vanadis is. A `--force` that fails part way through leaves the theme that was there intact. It
also replaces a theme file that is a symlink with a regular file, for the reason
[docs/config.md](config.md#an-output-that-is-a-symlink-is-replaced) gives about an output; a
`themes/` directory linked as one folded directory is unaffected, and one holding a link per
file is not.

### Provenance

`meta.author` comes across from the scheme, which
[docs/theme-format.md](theme-format.md#metadata) already calls provenance and already expects
a converter to fill. Where the file itself came from is the first line of the file, as a
comment.

```toml
# imported by vanadis from base16/nord
[meta]
format = 1
name = "Nord"
variant = "dark"
```

A cached scheme is named the way `search` prints it, which is the form that reaches it again.
A path is named by its absolute path and a URL by itself.

**A comment and not a field.** `[meta]`'s keys are fixed and an unknown one is an error, and
every table other than `[meta]` and `[text]` holds colour. There is nowhere to put it without
widening the format for something nothing reads.

`[text]` was the one place it would fit and is rejected. It is the author's namespace, a
template can read it as `{{text.source}}`, and a `text.source` somebody writes later would
collide with a value the tool wrote into a theme it does not own.

Nothing reads the line back. Re-importing is `--force`, which rewrites the whole file.

### Failure

The cache does not hold it:

```
error: the cache holds no scheme called `solarised`
run `vanadis search` to find one
```

Nothing has been cached and the source names a cached scheme:

```
error: no scheme cache under /home/ada/.cache/vanadis/schemes
run `vanadis remote update` to fetch it
```

A path and a URL reach neither of those: neither reads the cache, so neither needs one.

The theme is already there:

```
error: /home/ada/.config/vanadis/themes/nord.toml already exists
pass `--force` to write over it
```

The file is not a scheme this converts:

```
error: /home/ada/nord.yaml: cannot be converted
  caused by: the base16 palette does not carry `base0F`
```

## Failure

Nothing has been cached yet:

```
Error: no scheme cache under /home/ada/.cache/vanadis/schemes
run `vanadis remote update` to fetch it
```

`remote update` cannot reach the source:

```
error: cannot reach https://github.com/tinted-theming/schemes/archive/refs/heads/spec-0.11.tar.gz
  caused by: io: failed to lookup address information: Name or service not known
the cached schemes are unchanged; `vanadis search` still reads them
```

The source answers, but not with the archive:

```
error: https://github.com/tinted-theming/schemes/archive/refs/heads/spec-0.11.tar.gz answered 404
the cached schemes are unchanged; `vanadis search` still reads them
```

There is no `caused by:` line: the message already says everything. A status gets its own
message rather than folding into "cannot reach" because the URL answered: the machine is not
offline. The likeliest reason is that the branch the constant names, pinned in
[The source](#the-source), is gone.

The last line names what still works, because the common case for a failed fetch is a laptop
that is offline and already holds the collection. It is omitted when there is no cache, where
it would be false. It is why `remote update` prints its own failure instead of returning it:
no other command has a closing line that depends on what is on disk.

Paths in these messages are absolute and not shortened to `~`. The error is raised where the
home directory is not known, and threading it down to reach one message would put a
presentation concern into every signature on the way.

## Rejected alternatives

**An index built at update time.** Writing the five fields per scheme into one file at
extract time would let `search` read one file instead of 538. It is rejected because it
creates a second source of truth that can disagree with the directory beside it, and it has
to answer what happens when the index is missing, older than the schemes, or written by an
older version of vanadis. The saving it buys is on 538 small files that are already on the
local disk.

**Caching converted themes instead of the upstream files.** Running the converter at
`remote update` time and storing vanadis theme files would make `apply` work directly against
the cache. It bakes one version of the converter's decisions into the cache, so improving a
converter would silently disagree with everything already cached, and it discards the fields
the converter does not use before anyone has decided that they are not needed.

**Fetching on demand, when `search` finds no cache.** It would remove one step for a new
user. It also turns a command that reads local data into one that can hang on a network, and
puts a download in the way of a person who typed `search` to find out what they already have.
`remote update` is one command and it says what it does.

**Conditional requests, with the previous ETag.** The archive is 87 KB. Storing an ETag to
avoid re-downloading it adds state to keep valid for a saving smaller than the request that
checks for it.

## Left open

- Importing more than one scheme in a run. `import` takes one `SOURCE` and there is no
  `--all`. Somebody who wants the collection as themes wants a different command, with an
  answer for the 538 files it would write and for what `list` then looks like.
