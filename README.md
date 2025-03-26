# AIOSC - Artificial Intelligence Operating System Commander
<p align="center">
  <img src="aiosc.webp" alt="AIOSC Logo" />
</p>

## Overview
An intelligent CLI commander that leverages AI to manage and interact with operating system functions.
AIOSC provides a natural language interface to control and automate system operations through AI-powered command interpretation and execution.

## Features
- Natural language command processing
- System task automation
- Intelligent command suggestions
- Safe execution environment

## Configuration
### Location
AIOSC looks for configuration in the following locations:  
1. Configuration folder:
```
Linux:   /home/username/.config/aiosc/aiosc.config.json
Mac:     /Users/username/Library/Application Support/aiosc/aiosc.config.json
Windows: C:\Users\Username\AppData\Roaming\aiosc\aiosc.config.json
```
2. In the same directory where the binary is located:  
```
./aiosc
./aiosc.config.json
```

### Settings
- **debug**: Prints all requests to the LLM in the console.
- **api_addr**: API address to connect to the AI model server.
- **api_key**: API key to connect to the AI model server.
- **model**: AI model to use.
- **show_ai_commands_output**: Displays AI command outputs.
- **context_window_size**: Chat context window size (history).
- **shell_type**: Shell type (bash, zsh, cmd, PowerShell).
- **require_confirmation**: Requires confirmation before executing commands.

## LLM Server
Tested with:
- [OpenRouter AI](https://openrouter.ai)
- [LM Studio](https://lmstudio.ai)

## AI Models
Tested with:
- `deepseek/deepseek-chat-v3-0324:free` (OpenRouter AI)
- `qwen/qwen-2.5-coder-32b-instruct:free` (OpenRouter AI)
- `qwen2.5-coder-7b-instruct` (LM Studio)

### Configuration Format:  
`aiosc.config.json`
```json
{
  "debug": false,
  "api_addr": "https://openrouter.ai/api/v1",
  "api_key": "your-key",
  "model": "deepseek/deepseek-chat-v3-0324:free",
  "show_ai_commands_output": true,
  "context_window_size": 32,
  "shell_type": "bash",
  "require_confirmation": true
}
```
You can also pass configuration through environment variables:
```
AIOSC_DEBUG
AIOSC_API_ADDR
AIOSC_API_KEY
AIOSC_MODEL
AIOSC_SHOW_AI_COMMANDS_OUTPUT
AIOSC_CONTEXT_WINDOW_SIZE
AIOSC_SHELL_TYPE
AIOSC_REQUIRE_CONFIRMATION
```

## Status
The project is under active development.

## Contributing
I am open to cooperation to improve the functionality of this software. Pull requests are welcomed and highly encouraged!

## License
This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Author
Created by [tarkh](https://t.me/tarkhx)