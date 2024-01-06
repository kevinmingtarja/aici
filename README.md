# Artificial Intelligence Controller Interface (AICI)

The Artificial Intelligence Controller Interface (AICI) can be used to constrain output of an LLM in real time.
While the GPU is working on the next token of the output, the AICI VM can use the CPU to
compute a user-provided constraint on the next token.
This adds minimal latency to the LLM generation.

The AICI VM itself is built on top of [Wasmtime](https://wasmtime.dev/).
It takes [WebAssembly](https://webassembly.org/) (WASM) modules with a specific interface
(see below) and executes them in parallel while the LLM inference engine is working on the next token.

The WASM module can be generated in any language that can compile to WASM, but this project focuses on 
Rust for constraints (plus a Python package for interfacing with LLM runtime).

## Getting started

There are several levels at which you can use AICI.

* you can use the provided PyVM or SimpleVM on a remote server;
  no devcontainer is required in that case
* you can modify one of the provided VMs or build a new one;
  this typically requires rust, and the preferred way to work with it is to use the
  provided **AICI Client-side** devcontainer - it should work on any machine with Docker and VSCode
* if you want to run the inference server (rllm) locally, use the **AICI with CUDA** container;
  this requires a CUDA-capable GPU (currently only 8.0 (A100) is supported)
* finally, if you want to try the AICI integration with vLLM, use the 
  **AICI with CUDA and vLLM (experimental)** container

Each of the above containers takes longer than the previous one to build.

If you're not familiar with [devcontainers](https://containers.dev/),
you need to install the [Dev Containers VSCode extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
and from the command palette in VSCode select **Dev Containers: Reopen in Container...**.
It pops a list of available devcontainers, select the one you want to use.

### Interacting with server

To get started interacting with a cloud AICI server first export the API key.
If running local server, leave `AICI_API_BASE` unset.

```bash
export AICI_API_BASE="https://aici.azurewebsites.net/v1/#key=wht_..."
```

Now, use query the model with or without AICI VM:

```bash
./scripts/upload.sh --prompt "The answer to the ultimate question of life"
./scripts/upload.sh pyvm/samples/test.py
./scripts/upload.sh --vm target/wasm32-wasi/release/uppercase.wasm
```

Run `./scripts/upload.sh -h` to see usage info.

If the server is running with Orca-2 13B model,
you can also run tests with `pytest` for the DeclVM, or with `./scripts/test-pyvm.sh` for PyVM.


### Running local server

To run rLLM server, go to `rllm/` and run `./server.sh orca`.
This will run the inference server with Orca-2 13B model (which is expected by testcases).
You can also try other models, see [rllm/README.md](rllm/README.md) for details.

## Provided VMs

We provide several VMs that you can use directly or as base for your own VMs.

### Yes/no

The [yes/no VM](aici_abi/src/yesno.rs)
only allows the model to say "Yes" or "No" in answer to the question in the prompt.

```
$ ./scripts/sample-yesno.sh "Can orcas sing?"
+ cd aici_abi
+ cargo build --release
    Finished release [optimized + debuginfo] target(s) in 0.09s
+ ./scripts/upload.sh --vm target/wasm32-wasi/release/yesno.wasm --prompt 'Can orcas sing?'
upload module... 192kB -> 668kB id:0583a3ab
[0]: tokenize: "Yes" -> [8241]
[0]: tokenize: "No" -> [3782]
[0]: tokenize: "\n" -> [13]
[DONE]

[Prompt] Can orcas sing?

[Response] 
Yes
```

Note that the same effect can be achieved with PyVM and [10x less lines of code](pyvm/samples/yesno.py).
This is just for demonstration purposes.

```
$ ./scripts/upload.sh --prompt "Are dolphins fish?" pyvm/samples/yesno.py
...
No
```

### Uppercase

The [uppercase VM](aici_abi/src/uppercase.rs) shows usage of the `FunctionalRecognizer` interface.
It forces every 4th letter of the model output to be uppercase.

```
$ ./scripts/sample-uppercase.sh 
    Finished release [optimized + debuginfo] target(s) in 0.09s
upload module... 198kB -> 695kB id:c58fca60
[0]: user passed in 0 bytes
[0]: init_prompt: [1] ""
[0]: tokenize: "Here's a tweet:\n" -> [10605, 29915, 29879, 263, 7780, 300, 29901, 13]
[DONE]

[Prompt] 

[Response] Here's a tweet:
I'm SO EXCITED! I'm GoinG toBe aMom!I'm GoinG toHaVeA BaBy!
```

Again, this could be done with PyVM and a simple regex.

### PyVM

The [PyVM](pyvm) embeds [RustPython](https://github.com/RustPython/RustPython)
(a Python 3 language implementation) in the WASM module together with native
primitives for specific kinds of output constraints:
fixed token output, regexps, LR(1) grammars, substring constrains etc.
Python code is typically only used lightly, for gluing the primitives together,
and thus is not performance critical.

There are [several samples](pyvm/samples/) available.
The scripts use the [aici module](pyvm/aici-pylib/aici.py) to communicate with the AICI runtime
and use the native constraints.

To run a PyVM sample use:

```
$ ./scripts/upload.sh pyvm/samples/test.py
```

You will see the console output of the program.

### DeclVM

The [DeclVM](declvm/src/declvm.rs) exposes similar constraints
to PyVM, but the glueing is done via a JSON AST (Abstract Syntax Tree) and thus is
more restrictive.

There is no reason to use it as is, but it can be used as a base for other VMs.


## Architecture

This AICI runtime is implemented in the [aicirt](aicirt) crate, while the binary AICI interface 
is specified in the [aici_abi](aici_abi) crate.

The LLM engines are often implemented in Python, and thus the [pyaici](pyaici) Python packages provides
a class to spin up and communicate with `aicirt` process via POSIX shared memory and semaphores.
Using shared memory ensures there is very little work to be done on the Python side
(other than wrapping that memory as a tensor).

The (harness)[harness] folder contains samples for using aicirt with different LLM engines:
- [HuggingFace Transformers](harness/run_hf.py), run with `./scripts/hf.sh`
- [vLLM script](harness/run_vllm.py), run with `./scripts/vllm.sh`
- [vLLM REST server](harness/vllm_server.py), run with `./scripts/server.sh`;
  the REST server is compatible with OpenAI and adds an endpoint for uploading WASM modules;
  see [./scripts/upload.py](scripts/upload.py) for an example on how it can be used


```mermaid
graph TD
    User1 <-- HTTP --> vLLM
    User2 <-- HTTP --> vLLM
    UserN <-- HTTP --> vLLM["vLLM Server<br>(batching)"]
    vLLM <-- CUDA/pytorch --> GPU
    vLLM <-- POSIX SHM --> aicirt[AICI-runtime]
    aicirt <-- Sockets+SHM --> Worker1[Worker1<br>Running WASM]
    aicirt <-- Sockets+SHM --> Worker2[Worker2<br>Running WASM]
    aicirt <-- Sockets+SHM --> WorkerM[WorkerM<br>Running WASM]
```

```mermaid
sequenceDiagram
    actor User
    participant GPU
    participant vLLM
    participant aicirt as AICI-runtime
    vLLM -->> GPU: Model
    User -->> vLLM: Request (Prompt + WASM)
    vLLM -->>+ aicirt: Prompt + WASM
    aicirt -->>- vLLM: logit bias 1
    vLLM -->>+ GPU: Prompt
    vLLM -->> GPU: logit bias 1
    GPU -->> vLLM: token 1
    vLLM -->>+ aicirt: token 1
    vLLM -->> User: token 1
    aicirt -->>- vLLM: logit bias 2
    vLLM -->> GPU: logit bias 2
    GPU -->>- vLLM: token 2
    vLLM -->> User: token 2
```

Below is process structure.
* dotted arrow from A to B indicates that A sends requests to B (and gets responses)
* solid arrow from A to B indicates that A spawns (forks) B
* `spawner` is a special process, forked from `aicirt` at the beginning;
  for every user requests it spawns a top-level constraint VM and a `common state` process for handling shared state between
  all VMs for that request (all VMs for that user request can talk to the `common state` process)
* the top-level constraint can spawn more constraints, which can spawn yet more;
  `aicirt` has a direct connection to all these constraints though

```mermaid
graph TD
    vLLM ---> aicirt[AICI-runtime]
    vLLM -..-> aicirt
    aicirt -..-> spawner
    aicirt -..-> A0((A0))
    aicirt -..-> A1((A1))
    aicirt -..-> A2((A2))
    aicirt -..-> A3((A3))
    aicirt -..-> A4((A4))
    aicirt ---> spawner
    spawner --> A0
    spawner --> CommsA[Common state for A]
    subgraph User Request A
      A0 --> A1
      A0 --> A2
      A2 --> A3
      A2 --> A4
      A0 -..-> CommsA
      A1 -..-> CommsA
      A2 -..-> CommsA
      A3 -..-> CommsA
      A4 -..-> CommsA
    end
    aicirt -..-> B0((B0))
    aicirt -..-> B1((B1))
    spawner --> B0
    spawner --> CommsB[Common state for B]
    subgraph User Request B
      B0 -..-> CommsB
      B1 -..-> CommsB
      B0 --> B1
    end
```

## Security

- `aicirt` runs in a separate process, and can run under a different user than the LLM engine
- WASM modules are [sandboxed by Wasmtime](https://docs.wasmtime.dev/security.html)
- WASM only have access to [`aici_host_*` functions](aici_abi/src/host.rs),
  implemented in [hostimpl.rs](aicirt/src/hostimpl.rs)
- `aicirt` also exposes a partial WASI interface; however almost all the functions are no-op, except
  for `fd_write` which shims file descriptors 1 and 2 (stdout and stderr) to print debug messages

In particular, WASM modules cannot access the filesystem, network, or any other resources.
They also cannot spin threads or access any timers (this is relevant for Spectre/Meltdown attacks).

## Interfaces

### Low-level interface

Conceptually, the lowest level interface to AICI constraint is this:

```rust
type TokenId = u32;
type SeqId = u32;

trait AiciVm {
    /// Called with the initial prompt. ~1000ms time limit.
    fn init_prompt(prompt: Vec<TokenId>);

    /// Called before mid_process(), can fork or suspend. ~1ms.
    fn pre_process() -> enum {
        Stop,
        Continue, // Same as Fork { num_forks: 1 }
        Suspend,  // skip this generation round
        Fork { num_forks: u32 },
    }

    /// This is the main entry point for the module. ~20ms.
    fn mid_process(fork_group: Vec<SeqId>) -> enum {
        Stop,
        SampleWithBias { bias: Vec<f32> },
        Splice { backtrack: u32, ff_tokens: Vec<TokenId> }
    };

    /// Called after tokens are appended. ~1ms.
    fn post_process(tokens: Vec<TokenId>) -> enum { Stop, Continue };
}
```

Tokens depend on the tokenizer used (eg., for Llama there 32000 tokens, and for GPT-4 there is ~100k).

The actual binary interface is a bit more complicated, due
to limitations in passing values to and from WASM.
A WASM module instance is created for each token sequence.
Also, when the sequence forks (as in beam search), the module instance is cloned.
See the [AiciVm Rust trait](aici_abi/src/lib.rs) for details.

A number of functions are exposed to the WASM module.

First, there are functions for accessing the current tokenizer:

```rust
/// Given a byte sequence, return a sequence of token Ids.
fn tokenize_bytes(s: Vec<u8>) -> Vec<TokenId>;

/// Represents trie of all tokens in the current tokenizer.
impl TokTrie {
    /// Get Id for EOS token etc.
    fn special_token(tok: SpecialToken) -> TokenId;
    /// Number of tokens.
    fn vocab_size() -> usize;
    /// Convert token Id to bytes (often UTF-8 string).
    fn token(token: TokenId) -> Vec<u8>;
    /// Given a Recognizer, compute the set of allowed tokens. 
    fn compute_bias(rec: impl Recognizer) -> Vec<bool>;
}
```

Different forks in a sequence can communicate via shared variables:

```rust
/// This can be looked up in fork_group.
fn self_seq_id() -> SeqId;

trait VariableStorage {
    fn get(name: str) -> Option<Vec<u8>>;
    fn set(name: str, value: Vec<u8>);
    fn append(name: str, value: Vec<u8>);
}
```

Additionally, the `stdout` and `stderr` file descriptors are captured by the runtime
and returned to user when streaming results.

This interface may need to be extended in the future.

### Byte stack interface

The constraints are typically expressed on strings or bytes, not tokens.
To compute the set of tokens that match a string constraint, one needs go through all the possible tokens
and apply the constraint.
An efficient way to do this is walk a prefix tree (trie) of all tokens.
The `aici_abi` library implements this trie and exposes a way of filtering when provided with a constraints
implementing the [following interface](aici_abi/src/toktree.rs):

```rust
pub trait Recognizer {
    /// If `stack.top()` transitions via `byte` to `X`, execute `stack.push(X)`.
    fn push_byte(&mut self, byte: u8);
    /// for _ in 0..num { stack.pop() }
    fn pop_bytes(&mut self, num: usize);
    /// X = stack.top(); stack.empty(); stack.push(X)
    fn collapse(&mut self);
    /// check if stack.top() transitions via byte to a viable state
    fn byte_allowed(&mut self, byte: u8) -> bool;
    /// check if stack.top() transitions via tok to a viable state
    fn special_allowed(&mut self, tok: SpecialToken) -> bool;
    /// Called when iteration over the trie is finished
    /// Stack has exactly one element then.
    fn trie_finished(&mut self);
    /// This combines `push_byte` and `byte_allowed` into one function for performance.
    fn try_push_byte(&mut self, byte: u8) -> bool;
}
```

The `AiciRecognizer` struct converts `Recognizer` to `AiciVm`.

### Functional byte interface

The following interface can be transformed into `Recognizer` using `StackRecognizer` struct.

```rust
pub trait FunctionalRecognizer<S: Copy> {
    /// Initial state
    fn initial(&self) -> S;
    /// Extend the recognizer with given byte.
    fn append(&self, state: S, byte: u8) -> S;
    /// Check if given byte is allowed in given state.
    fn byte_allowed(&self, state: S, byte: u8) -> bool;
    /// Check if given special token is allowed in given state.
    fn special_allowed(&self, state: S, tok: SpecialToken) -> bool;
}
```

These three layers add up to about 40k of compiled code (WASM).

### Functional string interface

This is not implemented yet, but it could look like this:

```rust
pub trait StringRecognizer<S: Copy> {
    /// Initial state
    fn initial(&self) -> S;
    /// Extend the recognizer with given string.
    fn append(&self, state: S, suffix: &String) -> S;
    /// Return a set of allowed strings in given state.
    fn allowed(&self, state: S) -> Vec<String>;
    /// Check if given special token is allowed in given state.
    fn special_allowed(&self, state: S) -> Vec<SpecialToken>;
}
```


### Regular expressions

The `FunctionalRecognizer` interface is implemented for regular expressions.
The `S` type is the state of the DFA (Deterministic Finite Automaton) that recognizes the regular expression,
then `append()` and `byte_allowed()` are the standard DFA operations,
while `special_allowed()` is only implemented for end-of-sequence token
(which is allowed when the current state is accepting).

### LR(1) grammars

The `Recognizer` interface is implemented for LR(1) grammars and DFA-based lexers.

The grammar uses inline syntax for the lexer:
- `"keyword"` or `'keyword'` for keywords; any string works, eg. `"+="`, `"while"`, ...
- `"/.../"` or `'/.../'` for regular expressions; you cannot have both `'` and `"` in the regex
Special `SKIP` rule is used to indicate tokens that need to be skipped by the LR(1) parser (eg., whitespace and comments)

The lexer has a DFA which recognizes all regexps and keywords
(a big disjunction, but with additional machinery to disambiguate between different branches).
It goes byte by byte, until the DFA gets to a dead state (from which no match is possible).
Then it goes back one byte and checks for match.
It prefers keywords over regexps.
If no match is found, an error is reported, which requires careful design of the lexical part of the grammar
(eg., see how the `white-space` rule below is prefix of the `pre-processor` rule).

For example, this is fragment of [grammar for C](./grammars/c.y):

```yacc
%start translation_unit
%%

SKIP
    : "//\*[^*]*\*+([^/*][^*]*\*+)*//"  // block comment
    | "///.*/"                          // line comment
    | "/\n[ \t\v\f]*#(.*\\\n)*.*/"      // pre-processor
    | "/\n?[ \t\v\f]*/"                 // white-space
    ;

IDENTIFIER: "/[a-zA-Z_][0-9a-zA-Z_]*/" ;

CONSTANT
        : "/0[xX][0-9a-fA-F]+[uUlL]*?/"
        | "/0[0-9]+[uUlL]*?/"
        ;

STRING_LITERAL: '/"(\\.|[^\\"])*"/' ;

primary_expression
    : IDENTIFIER
    | CONSTANT
    | STRING_LITERAL
    | "(" expression ")"
    ;

// ...

enum_specifier
    : "enum" "{" enumerator_list "}"
    | "enum" IDENTIFIER "{" enumerator_list "}"
    | "enum" IDENTIFIER
    ;

// ...

translation_unit
    : external_declaration
    | translation_unit external_declaration
    ;
```

## Contributing

This project welcomes contributions and suggestions.  Most contributions require you to agree to a
Contributor License Agreement (CLA) declaring that you have the right to, and actually do, grant us
the rights to use your contribution. For details, visit https://cla.opensource.microsoft.com.

When you submit a pull request, a CLA bot will automatically determine whether you need to provide
a CLA and decorate the PR appropriately (e.g., status check, comment). Simply follow the instructions
provided by the bot. You will only need to do this once across all repos using our CLA.

This project has adopted the [Microsoft Open Source Code of Conduct](https://opensource.microsoft.com/codeofconduct/).
For more information see the [Code of Conduct FAQ](https://opensource.microsoft.com/codeofconduct/faq/) or
contact [opencode@microsoft.com](mailto:opencode@microsoft.com) with any additional questions or comments.

## Trademarks

This project may contain trademarks or logos for projects, products, or services. Authorized use of Microsoft 
trademarks or logos is subject to and must follow 
[Microsoft's Trademark & Brand Guidelines](https://www.microsoft.com/en-us/legal/intellectualproperty/trademarks/usage/general).
Use of Microsoft trademarks or logos in modified versions of this project must not cause confusion or imply Microsoft sponsorship.
Any use of third-party trademarks or logos are subject to those third-party's policies.
