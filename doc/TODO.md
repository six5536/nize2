- IDEA: File/data handles for LLMS via MCP tools - a standard (uri?)


/zen.brainstorm This project will be an amazing tool.

The purpose is to give LLMs the ability to share files and other data between MCP servers and other code they call.

It will consist of:

- ai-url: A set of (functionally identical) library modules written initially in Rust, but later in TypeScript, Go and Python. These libraries define an extension of URIs so they can become handles to documents and other data that LLMs can pass between their tools so they can move files and data around between their tools securely without actually having to have access to it. For example, to attach a local file to an email: a `file MCP` is asked for a `aiurl` of a specific file. It gets it and gives it to an `email MCP` (local) that can decode the `aiurl` read the file, and attach it to the required email. You can imagine limitless other uses.
- The API will be defined in .tsp files and the interface code for each library will be generated from the .tsp using code-generation.
- The library will be able to read data via modules. The first will be filesystem, but others could be 'http-download' or anything else
- The aiurl should be partially human (LLM) readable (title), with an encoded part that is easy on LLMs.
- There needs to be a way to manage permission(!) this is hard!