# CQ-editor

[![Build status](https://ci.appveyor.com/api/projects/status/g98rs7la393mgy91/branch/master?svg=true)](https://ci.appveyor.com/project/adam-urbanczyk/cq-editor/branch/master)
[![codecov](https://codecov.io/gh/CadQuery/CQ-editor/branch/master/graph/badge.svg)](https://codecov.io/gh/CadQuery/CQ-editor)
[![Build Status](https://dev.azure.com/cadquery/CQ-editor/_apis/build/status/CadQuery.CQ-editor?branchName=master)](https://dev.azure.com/cadquery/CQ-editor/_build/latest?definitionId=3&branchName=master)
[![DOI](https://img.shields.io/badge/DOI-10.5281%2Fzenodo.3955112-blue.svg)](https://doi.org/10.5281/zenodo.3955112)
[![Patreon](https://badgen.net/badge/icon/patreon?icon=patreon&label)](https://www.patreon.com/jmwright)
[![Liberapay](https://badgen.net/badge/icon/liberapay?icon=liberapay&label)](https://liberapay.com/jmwright)

CadQuery GUI editor based on PyQT that supports Linux, Windows and Mac.

![CQ-editor screenshot](https://github.com/CadQuery/CQ-editor/raw/master/screenshots/screenshot4.png)
Additional screenshots are available in [the wiki](https://github.com/CadQuery/CQ-editor/wiki#screenshots).

## Notable features

* Automatic code reloading - you can use your favourite editor
* OCCT based
* Graphical debugger for CadQuery scripts
  * Step through script and watch how your model changes
* CadQuery object stack inspector
  * Visual inspection of current workplane and selected items
  * Insight into evolution of the model
* Export to various formats
  * STL
  * STEP
* AI Chat Assistant *(opt-in)* — generate and iterate on CadQuery models using natural language via any OpenAI-compatible API

## AI Chat Assistant

An optional dockable panel that lets you describe what you want to model and receive a complete, runnable CadQuery script in response. The current editor script is automatically sent as context with every message, so the model edits your existing code rather than starting from scratch.

The panel is disabled by default. No network requests are made unless you explicitly enable it and send a prompt.

### Privacy

Every prompt you send includes your current editor script and is transmitted to the API endpoint you configure. Before sending the first message, the panel will show a one-time confirmation dialog that lists the endpoint. You can change or review the endpoint at any time in **Edit -> Preferences -> AI Assistant**.

The API key is stored in the system keychain if the `keyring` package is installed. Otherwise it falls back to plaintext in the local preferences file. Install `keyring` for secure storage:

```bash
pip install keyring
```

### Installation

The panel requires the `openai` package and optionally `keyring` for secure key storage. You can install all optional AI Assistant dependencies in one command using the project's standard extras group:

```bash
pip install .[ai]
```

Or install them manually:

```bash
pip install openai keyring
```

### Configuration

Open **Edit -> Preferences -> AI Assistant** and set the following:

| Setting | Description | Default |
|---|---|---|
| Enabled | Show the AI Assistant dock panel | false |
| Provider / Base URL | OpenAI-compatible API endpoint | `https://api.openai.com/v1` |
| Model | Model identifier | `gpt-4o` |
| API Key | Provider API key | *(empty)* |
| Auto-run after insert | Re-render the model after inserting code | true |

**Supported providers**

| Provider | Base URL |
|---|---|
| OpenAI | `https://api.openai.com/v1` |
| OpenRouter | `https://openrouter.ai/api/v1` |
| Ollama (local) | `http://localhost:11434/v1` |
| Any OpenAI-compatible API | your custom endpoint |

Once Enabled is checked, the panel appears as a dock on the right side. It can also be toggled from **Tools -> AI Assistant** or the **View** menu.

### Usage

Type a description in the prompt box and press Enter or click Send. The assistant always replies with a complete `python` fenced code block. Once a response arrives, click **Insert & Run** to load the code into the editor and re-render the model.

If the response contains no code block — for example, the model returned a clarifying question — the Insert button stays disabled and an informational message is shown in the chat.

**Generating a new model**

```
Create a hollow cylinder, 40mm outer diameter, 30mm inner diameter, 80mm tall
```

**Iterating on an existing model**

```
Add a 2mm fillet to all vertical edges
```

```
Change the wall thickness to 5mm and add M3 mounting holes at each corner
```

**Fixing errors (One-Tap Self-Healing)**

If the rendered script throws an error, the traceback appears in the **Current traceback** panel. You don't need to copy and paste anything:

* Simply click the vibrant **✨ Auto-Fix with AI** button at the top of the traceback pane.
* The editor will instantly capture the traceback, line numbers, and crash snippet, open the AI Assistant, and request a fix.
* Once the corrected code is returned, it is **automatically inserted back into the editor and re-rendered in one tap!**

**Resetting the conversation**

Click **Clear Chat** to discard the conversation history and start a new session. The privacy consent is also reset, so the notice will appear again on the next send.

### Notes

* **Model Quality Matters**: For the best results, use high-capability frontier models. Larger models are vastly superior at precise geometric reasoning, spatial awareness, and successfully resolving CadQuery API errors compared to smaller or lightweight local models.
* Conversation history is capped at 10 turn pairs. Older messages are dropped automatically to limit token usage.
* Models that produce consistent fenced code blocks work best.
* The panel shuts down its background thread cleanly when CQ-editor closes, even if a request is in progress.

## Documentation

Documentation is available in [the wiki](https://github.com/CadQuery/CQ-editor/wiki). Topics covered are the following.

* [Installation](https://github.com/CadQuery/CQ-editor/wiki/Installation)
* [Usage](https://github.com/CadQuery/CQ-editor/wiki/Usage)
* [Configuration](https://github.com/CadQuery/CQ-editor/wiki/Configuration)

## Getting Help

For general questions and discussion about CQ-editor, please create a [GitHub Discussion](https://github.com/CadQuery/CQ-editor/discussions).

## Reporting a Bug

If you believe that you have found a bug in CQ-editor, please ensure the following.

* You are not running a CQ-editor fork, as these are not always synchronized with the latest updates in this project.
* You have searched the [issue tracker](https://github.com/CadQuery/CQ-editor/issues) to make sure that the bug is not already known.

If you have already checked those things, please file a [new issue](https://github.com/CadQuery/CQ-editor/issues/new) with the following information.

* Operating System (type, version, etc) - If running Linux, please include the distribution and the version.
* How CQ-editor was installed.
* Python version of your environment (unless you are running a pre-built package).
* Steps to reproduce the bug.
