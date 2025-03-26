# AIOSC - Artificial Intelligence Operating System Commander

An intelligent CLI Commander that leverages AI to manage and interact with operating system functions.

## Overview
AIOSC provides a natural language interface to control and automate system operations through AI-powered command interpretation and execution.

## Features
- Natural language command processing
- System task automation
- Intelligent command suggestions
- Safe execution environment

## Config
`aiosc.config.json`
```
{
  "debug": true,
  "useOpenRouter": false,
  "openRouterApiKey": "your-key",
  "openRouterModel": "some-model",
  "lmStudioApiAddr": "http://localhost:1234/v1",
  "lmStudioModel": "some-local-model",
  "showAiCommandsOutput": false,
  "contextWindowSize": 10,
  "shellType": "powershell",
  "requireConfirmation": false
}
```
or `Envs`
```
AIOSC_DEBUG
AIOSC_USE_OPENROUTER
AIOSC_OPENROUTER_API_KEY
AIOSC_OPENROUTER_MODEL
AIOSC_LM_STUDIO_API_ADDR
AIOSC_LM_STUDIO_MODEL
AIOSC_SHOW_AI_COMMANDS_OUTPUT
AIOSC_CONTEXT_WINDOW_SIZE
AIOSC_SHELL_TYPE
AIOSC_REQUIRE_CONFIRMATION
```

## Status
Project is under active development.
