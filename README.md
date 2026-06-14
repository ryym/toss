# Toss ― A modern terminal pager

Toss is a terminal pager like `less`. Toss focuses on the following features:

- **Smooth scroll** - Animate scrolling with easing
- **Incremental search** - Highlight matches as you type
- **Sticky header** - Pin a pattern-matched header line while scrolling

![](./assets/example-git-delta.giff)

## Status

- In active development
- Not a direct replacement of `less`; Toss currently lacks many features of `less`

## Installation

```bash
git clone https://github.com/ryym/toss
cd toss
cargo install --path .
```

## Usage Examples

### View a Git commit with [Delta][delta]

[delta]: https://github.com/dandavison/delta

```bash
# View a commit with Delta and Toss.
git show @ | delta \
      --file-modified-label 'updated:' \
      --pager 'toss -F \
         --header 1 \
         --heading "^(added:|updated:|renamed:|removed:)" --heading-lines 2'
# --header 1
#   Always display the first line, which is a commit hash.
# --heading "..." --heading-lines 2
#   Make each file name header sticky by pattern-matching.
```

You can make this a default pager in Git by specifying the above `delta` command as a `core.pager` of your Git.

### View files with [Bat][bat]

[bat]: https://github.com/sharkdp/bat

```bash
# Bat config (https://github.com/sharkdp/bat#configuration-file)
--theme=TwoDark
--style='header,numbers,grid,changes'
# ...
--pager 'toss -F --heading "─┬─" --heading-lines 3'
```
