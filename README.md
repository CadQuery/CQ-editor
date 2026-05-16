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
* **🤖 AI Chat Assistant** *(opt-in)* — describe your model in plain English and get runnable CadQuery code instantly, with iterative editing support

## 🤖 AI Chat Assistant

CQ-editor includes an optional **AI Chat Assistant** panel that lets you generate and iterate on CadQuery models using natural language. It works with any OpenAI-compatible API provider — including OpenAI, OpenRouter, Anthropic (via OpenRouter), and local models via Ollama.

### Setup

**1. Install the optional dependency**

```bash
pip install openai
```

> CQ-editor starts and works normally without this package. The AI panel is fully opt-in.

**2. Configure your API key and model**

Open **Edit → Preferences → AI Assistant** and fill in:

| Setting | Description | Example |
|---|---|---|
| Enabled | Turn the panel on/off | ✓ |
| Provider / Base URL | API endpoint | `https://api.openai.com/v1` |
| Model | Model identifier | `gpt-4o`, `o3`, `claude-sonnet-4-5` |
| API Key | Your provider API key | `sk-...` |
| Auto-run after insert | Re-render the model immediately after inserting code | ✓ |

**Supported providers (via Base URL)**

| Provider | Base URL |
|---|---|
| OpenAI | `https://api.openai.com/v1` |
| OpenRouter | `https://openrouter.ai/api/v1` |
| Local Ollama | `http://localhost:11434/v1` |
| Any OpenAI-compatible API | your custom endpoint |

**3. Open the panel**

Go to **Tools → 🤖 AI Assistant**, or toggle it from the **View** menu like any other dock panel.

### Usage

**Generating a new model from scratch**

1. Type a description in the prompt box, for example:
   ```
   Create a hollow cylinder, 40mm outer diameter, 30mm inner diameter, 80mm tall
   ```
2. Press **Enter** or click **Send**.
3. When the AI replies with code, click **Insert & Run**.
4. The model appears instantly in the 3D viewer.

**Iterating on an existing model**

The AI automatically receives your **current editor script as context** with every message. This means you can say:

```
Add a 2mm fillet to all vertical edges
```

or

```
Change the wall thickness to 5mm and add mounting holes at each corner
```

and the AI will modify the existing code rather than starting from scratch.

**Typical workflow**

```
[You]  Make a rectangular enclosure 100x60x40mm with a 2mm wall thickness
[AI]   import cadquery as cq
       result = (
           cq.Workplane("XY")
           .box(100, 60, 40)
           .shell(-2)
       )
       show_object(result)
[You click Insert & Run → model renders in viewer]

[You]  Add four M3 mounting holes at the corners, 5mm from each edge
[AI]   <updated full script with mounting holes>
[You click Insert & Run → model updates]
```

### Tips

* The best models for CadQuery code generation are currently **o3**, **gpt-4o**, and **Gemini 2.5 Pro** (via OpenRouter).
* If the generated code has an error, the traceback appears in the **Current traceback** panel — you can copy it and paste it back into the chat: *"Fix this error: ..."*.
* Click **Clear Chat** to reset the conversation history and start a new model.
* The API key is stored in local preferences only and is never sent anywhere other than your configured provider endpoint.

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
