# Clap provider

<!-- TOC GFM -->

* [Introduction](#introduction)
  * [1. Non-pure-async provider](#1-non-pure-async-provider)
  * [2. Pure async provider](#2-pure-async-provider)
* [Provider arguments](#provider-arguments)
* [Create non-pure-async provider](#create-non-pure-async-provider)
* [Create pure async provider](#create-pure-async-provider)
  * [Non-RPC based](#non-rpc-based)
  * [RPC-based](#rpc-based)
* [Register provider](#register-provider)
* [FAQ](#faq)
  * [How to add the preview support for my provider?](#how-to-add-the-preview-support-for-my-provider)

<!-- /TOC -->

### Introduction

The provider of vim-clap is actually a Dict that specifies the action of your move in the input window. The idea is simple, once you have typed something, the `source` will be filtered or a job will be spawned, and then the result retrived later will be shown in the dispaly window.

There are generally two kinds of providers in vim-clap.

#### 1. Non-pure-async provider

suitable for these which are able to collect all the items in a short time, e.g., open buffers, command history.It will run sync if the source is not large. But it's also able to deal with the list that is huge, let's say 100,000+ lines/items, in which case vim-clap will choose to run the external filter in async. In a word, vim-clap can always be fast responsive. What's more, it's extremely easy to introduce a new non-pure-async clap provider as vim-clap provides the default implementation of `on_typed` and `source_async`.

Caveat: if you have some synchronous operations in `source`, e.g., read multiple files, ensure it won't slow clap down, as in which case the default implementation of `source_async` won't help.

#### 2. Pure async provider

suitable for the time-consuming jobs, e.g., grep a word in a directory. Checkout out [grep provider](autoload/clap/provider/grep.vim).

### Provider arguments

```vim
:Clap [provider_id_or_alias] [++opt] [+opt]
```

All the opts are accessible via `g:clap.context[opt]`.

The form of `[++opt]` is `++{optname}={value}`, where {optname} is one of:

  - `++externalfilter=fzf` or `++ef=fzf`.

`[+opt]` is used for the bool arguments:

 - `+async`

`Clap! [provider_id_or_alias]` is equal to `Clap [provider_id_or_alias] +async`.

`++opt` and `+opt` will be stored in the Dict `g:clap.context`, the rest arguments will be stored in a List of String `g:clap.provider.args`.

### Create non-pure-async provider

For the non-pure-async providers, you could run it in async or sync way. By default vim-clap will choose the best strategy, running async for the source consisted of 5000+ lines or otherwise run it in sync way. [See the discussion about the non-pure-async providers](https://github.com/liuchengxu/vim-clap/issues/17#issue-501470657).

Field                 | Type                | Required      | Has default implementation
:----                 | :----               | :----         | :----
`sink`                | String/Funcref      | **mandatory** | No
`sink*`               | Funcref             | optional      | No
`source`              | String/List/Funcref | **mandatory** | No
`source_type`         | Number              | optional      | No
`source_async`        | String              | optional      | **Yes**
`filter`              | Funcref             | **mandatory** | **Yes**
`on_typed`            | Funcref             | **mandatory** | **Yes**
`on_move`             | Funcref             | optional      | No
`on_enter`            | Funcref             | optional      | No
`on_exit`             | Funcref             | optional      | No
`support_open_action` | Bool                | optional      | **Yes** if the `sink` is `e`/`edit`/`edit!`
`enable_rooter`       | Bool                | Optional      | No
`syntax`              | String              | Optional      | No
`prompt_format`       | String              | Optional      | No
`init`                | Funcref             | Optional      | **Yes**

- `sink`:
  - String: vim command to handle the selected entry.
  - Funcref: reference to function to process the selected entry.

- `sink*`: similar to `sink`, but takes the list of multiple selected entries as input.

- `source`:
  - List: vim List as input to vim-clap.
  - String: external command to generate input to vim-clap (e.g. `find .`).
  - Funcref: reference to function that returns a List to generate input to vim-clap.

- `source_type`: type of `source`, vim-clap can detect it itself, but could be slow in some edge cases, e.g., `blines` for a file having 1 million lines. Setting this property explicitly can save the time for checking the source type.
  - `g:__t_string`
  - `g:__t_list`
  - `g:__t_func_string`
  - `g:__t_func_list`

- `source_async`: String, job command to filter the items of `source` based on the external tools. The default implementation is to feed the output of `source` into the external fuzzy filters and then display the filtered result, which could have some limitations, e.g., the matched input is not highlighted.

- `filter`: given what you have typed, use `filter(entry)` to evaluate each entry in the display window, when the result is zero remove the item from the current result list. The default implementation is to match the input using vim's regex.

- `on_typed`: reference to function to filter the `source`.

- `on_move`: when navigating the result list, can be used for the preview purpose, see [clap/provider/colors](autoload/clap/provider/colors.vim).

- `on_enter`: when entering the clap window, can be used for recording the current state.

- `on_exit`: can be used for restoring the state on start.

- `enable_rooter`: try to run the `source` from the project root.

- `syntax`: used to set the syntax highlight for the display buffer easier. `let s:provider.syntax = 'provider_syntax'` is equal to `let s:provider.on_enter = { -> g:clap.display.setbufvar('&syntax', 'provider_syntax')}`.

- `prompt_format`: used for showing some dynamic information, checkout [autoload/clap/provider/tags.vim](autoload/clap/provider/tags.vim) for the usage. Don't forget to call `clap#spinner#refresh()` to reveal the changes after setting a new `prompt_format` in the provider.

- `init`: used for initializing the display window.

You have to provide `sink` and `source` option. The `source` field is indispensable for a synchronous provider. In another word, if you provide the `source` option this provider will be seen as a sync one, which means you could use the default `on_typed` implementation of vim-clap.

### Create pure async provider

#### Non-RPC based

Everytime your input is changed, a new job will be spawned.

Field                 | Type    | Required      | Has default implementation
:----                 | :----   | :----         | :----
`sink`                | funcref | **mandatory** | No
`on_typed`            | funcref | **mandatory** | No
`on_move`             | funcref | optional      | No
`on_enter`            | funcref | optional      | No
`converter`           | funcref | optional      | No
`jobstop`             | funcref | **mandatory** | **Yes** if you use `clap#dispatcher#job_start(cmd)`
`support_open_action` | Bool    | optional      | **Yes** if the `sink` is `e`/`edit`/`edit!`
`enable_rooter`       | Bool    | Optional      | No
`prompt_format`       | String  | Optional      | No
`syntax`              | String  | Optional      | No

- `on_typed`: reference to function to spawn an async job.
- `converter`: reference to function to convert the raw output of job to another form, e.g., prepend an icon to the grep result, see [clap/provider/grep.vim](autoload/clap/provider/grep.vim).
- `jobstop`: Reference to function to stop the current job of an async provider. By default you could utilize `clap#dispatcher#job_start(cmd)` to start a new job, and then the job stop part will be handled by vim-clap as well, otherwise you'll have to take care of the `jobstart` and `jobstop` on your own.

You must provide `sink`, `on_typed` option. It's a bit of complex to write an asynchornous provider, you'll need to prepare the command for spawning the job and overal workflow, although you could use `clap#dispatcher#job_start(cmd)` to let vim-clap deal with the job control and display update. Take [clap/provider/grep.vim](autoload/clap/provider/grep.vim) for a reference.

#### RPC-based

The RPC service will be started on initializing the display window when this kind of provider is invoked. Everytime your input is changed, the filtering happens or the request will be send the stdio RPC server powered by the Rust binary `maple`. The `source_typ` has to be `g:__t_tpc`. Additional properties for the provider are:

Field           | Type    | Required      | Has default implementation
:----           | :----   | :----         | :----
`on_no_matches` | funcref | optional      | No
`tab_action`    | funcref | optional      | No
`bs_action`     | funcref | optional      | No
`init`          | funcref | **mandatory** | No


This kind of provider requires you to be experienced in VimScript and Rust. Checkout the source code [autoload/clap/provider/filer.vim](autoload/clap/provider/filer.vim) and [src/rpc.rs](src/rpc.rs) directly.

### Register provider

Vim-clap will try to load the providers with such convention:

- vimrc

Define `g:clap_provider_{provider_id}` in your vimrc, e.g.,

```vim
" `:Clap quick_open` to open some dotfiles quickly.
let g:clap_provider_quick_open = {
      \ 'source': ['~/.vimrc', '~/.spacevim', '~/.bashrc', '~/.tmux.conf'],
      \ 'sink': 'e',
      \ }
```

- autoload

`g:clap#provider#{provider_id}#`. See `:h autoload` and [clap/provider](autoload/clap/provider).

### FAQ

#### How to add the preview support for my provider?

Use `on_move()` and `g:clap.preview.show([lines])`, ensure it always runs fast.
