# cq_worker

The CadQuery execution backend of [CQ-editor](https://github.com/igors93/CQ-editor).
The editor starts it as `python -m cq_worker` and talks to it over
stdin/stdout, one JSON object per line. The package is embedded in the
CQ-editor executable, so users never install it themselves.

Documentation lives in the repository's `docs/` folder:

* [Architecture](../docs/development/architecture.md#python-side-workercq_worker): what each module does
* [Worker protocol](../docs/development/protocol.md): requests and messages
* [Testing](../docs/development/testing.md): running `pytest tests/worker`
